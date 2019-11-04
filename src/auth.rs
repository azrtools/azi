use std::cell::RefCell;
use std::env::var_os;
use std::fs::create_dir_all;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use base64::decode;
use dirs::home_dir;
use humantime::format_rfc3339_seconds;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::from_reader;
use serde_json::from_slice;
use serde_json::from_value;
use serde_json::to_value;
use serde_json::Value;

use crate::error::AppError::AccessTokenFileError;
use crate::utils::Result;

const ACCESS_TOKENS_PATH: &'static str = ".azure/accessTokens.json";
const AZURE_PROFILE_PATH: &'static str = ".azure/azureProfile.json";
const DEFAULT_EXPIRATION: u64 = 60 * 60;

pub struct AccessTokenFile {
    path: PathBuf,
    entries: RefCell<Vec<AccessTokenFileEntry>>,
    tenant: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessTokenFileEntry {
    #[serde(rename = "_clientId")]
    pub client_id: String,
    pub resource: String,
    pub access_token: String,
    pub refresh_token: String,

    #[serde(rename = "_authority")]
    authority: String,
    token_type: String,
    expires_on: String,
    user_id: String,

    #[serde(rename = "isMRRT")]
    is_mrrt: bool,
}

impl AccessTokenFile {
    pub fn new(tenant: Option<&str>) -> Result<AccessTokenFile> {
        let path = if let Some(ref path) = var_os("AZURE_ACCESS_TOKEN_FILE") {
            PathBuf::from(path)
        } else if let Some(ref home_dir) = home_dir() {
            home_dir.join(ACCESS_TOKENS_PATH).to_owned()
        } else {
            return Err(AccessTokenFileError.into());
        };

        let tenant = match tenant {
            Some(tenant) => Some(tenant.to_owned()),
            None => Self::read_tenant()?,
        };

        let entries = RefCell::new(Self::read_entries(&path, &tenant)?);

        trace!("Read access token entries: {:#?}", entries);

        Ok(AccessTokenFile {
            path,
            tenant,
            entries,
        })
    }

    fn read_tenant() -> Result<Option<String>> {
        if let Some(ref home_dir) = home_dir() {
            let path = home_dir.join(AZURE_PROFILE_PATH);
            if let Some(subscriptions) = Self::read_file(&path)?["subscriptions"].as_array() {
                for subscription in subscriptions {
                    if subscription["isDefault"] == Value::Bool(true) {
                        if let Some(tenant) = subscription["tenantId"].as_str() {
                            return Ok(Some(tenant.to_owned()));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    fn read_entries(path: &Path, tenant: &Option<String>) -> Result<Vec<AccessTokenFileEntry>> {
        if let Some(arr) = Self::read_file(&path)?.as_array() {
            Ok(Self::parse_arr(arr, tenant))
        } else {
            Ok(vec![])
        }
    }

    fn read_file(path: &Path) -> Result<Value> {
        if path.exists() {
            let file = File::open(&path)?;
            let reader = Self::skip_bom(BufReader::new(file))?;
            match from_reader(reader) {
                Err(e) => {
                    trace!("Failed to parse file: {}", path.display());
                    Err(e.into())
                }
                Ok(value) => Ok(value),
            }
        } else {
            debug!("File not found: {}", path.display());
            Ok(Value::Null)
        }
    }

    fn skip_bom(mut reader: BufReader<File>) -> Result<BufReader<File>> {
        let buf = reader.fill_buf()?;
        if buf.len() >= 3 && buf[0] == 0xEF && buf[1] == 0xBB && buf[2] == 0xBF {
            reader.read_exact(&mut [0; 3])?;
        }
        Ok(reader)
    }

    fn parse_arr(arr: &Vec<Value>, tenant: &Option<String>) -> Vec<AccessTokenFileEntry> {
        let mut entries = vec![];
        for entry in arr {
            if let Some(e) = from_value(entry.clone()).ok() {
                let e: AccessTokenFileEntry = e;
                let match_tenant = match (tenant, e.tenant()) {
                    (Some(t1), Some(t2)) => t1 == t2,
                    (Some(_), None) => false,
                    (None, _) => true,
                };
                if match_tenant {
                    entries.push(e);
                }
            }
        }
        entries
    }

    pub fn any_entry(&self) -> Option<AccessTokenFileEntry> {
        self.entries.borrow().first().map(|e| e.clone())
    }

    pub fn find_entry(&self, resource: &str) -> Option<AccessTokenFileEntry> {
        for entry in self.entries.borrow().iter() {
            if entry.resource == resource {
                return Some(entry.clone());
            }
        }
        debug!("Did not find a matching entry: {}", resource);
        None
    }

    pub fn update_entry(&self, entry: &AccessTokenFileEntry) -> Result<()> {
        let mut json = match Self::read_file(self.path.as_ref())? {
            Value::Null => Value::Array(vec![]),
            json => json,
        };
        if let Some(arr) = json.as_array_mut() {
            let mut updated = false;
            for e in arr.iter_mut() {
                if entry.match_key(e) {
                    if let Some(existing) = e.as_object_mut() {
                        existing.insert(
                            "accessToken".to_owned(),
                            Value::String(entry.access_token.clone()),
                        );
                        existing.insert(
                            "refreshToken".to_owned(),
                            Value::String(entry.refresh_token.clone()),
                        );
                        existing.insert(
                            "expiresOn".to_owned(),
                            Value::String(entry.expires_on.clone()),
                        );
                        debug!("Updated token");
                        updated = true;
                    }
                }
            }

            if !updated {
                arr.push(to_value(entry)?);
                debug!("Added new token");
            }

            if let Some(parent) = self.path.parent() {
                create_dir_all(parent)?;
            }

            let file = File::create(&self.path)?;
            serde_json::to_writer(&file, arr)?;

            debug!("Updated access token file");

            let entries = Self::parse_arr(arr, &self.tenant);
            self.entries.replace(entries);

            trace!("Updated access token entries: {:#?}", self.entries);
        } else {
            debug!("JSON is not an array, skipping update!");
        }

        Ok(())
    }
}

impl AccessTokenFileEntry {
    pub fn parse(json: Value) -> Result<AccessTokenFileEntry> {
        macro_rules! to_str {
            ($a:expr) => {
                $a.as_str().map(str::to_owned).ok_or(AccessTokenFileError)
            };
        }

        fn decode_token(token: &str) -> Result<Value> {
            if let (Some(start), Some(end)) = (token.find('.'), token.rfind('.')) {
                if start < end {
                    let decoded = decode(&token[start + 1..end])?;
                    return Ok(from_slice(decoded.as_slice())?);
                }
            }
            return Err(AccessTokenFileError.into());
        }

        let access_token = to_str!(json["access_token"])?;

        let access_token_decoded = decode_token(&access_token)?;
        let client_id = to_str!(access_token_decoded["appid"])?;
        let authority = Self::create_authority(&to_str!(access_token_decoded["tid"])?);
        let user_id = to_str!(access_token_decoded["unique_name"])?;

        let expires_on = if let Some(expires_on) = json["expires_on"].as_u64() {
            format_rfc3339_seconds(UNIX_EPOCH + Duration::from_secs(expires_on))
        } else if let Some(expires_on) = json["expires_on"].as_str() {
            let seconds = expires_on.parse::<u64>()?;
            format_rfc3339_seconds(UNIX_EPOCH + Duration::from_secs(seconds))
        } else {
            format_rfc3339_seconds(SystemTime::now() + Duration::from_secs(DEFAULT_EXPIRATION))
        };

        Ok(AccessTokenFileEntry {
            resource: to_str!(json["resource"])?,
            refresh_token: to_str!(json["refresh_token"])?,
            token_type: to_str!(json["token_type"])?,
            expires_on: expires_on.to_string(),
            is_mrrt: true,
            client_id,
            access_token,
            authority,
            user_id,
        })
    }

    fn create_authority(tenant: &str) -> String {
        format!("https://login.microsoftonline.com/{}", tenant)
    }

    pub fn with_token(&self, access_token: String, resource: String) -> AccessTokenFileEntry {
        AccessTokenFileEntry {
            access_token,
            resource,
            ..(self.clone())
        }
    }

    pub fn with_tenant(&self, tenant: &str) -> AccessTokenFileEntry {
        AccessTokenFileEntry {
            authority: Self::create_authority(tenant),
            ..(self.clone())
        }
    }

    pub fn tenant(&self) -> Option<&str> {
        match self.authority.rfind("/") {
            Some(pos) => Some(&self.authority[pos + 1..]),
            None => None,
        }
    }

    fn match_key(&self, json: &Value) -> bool {
        json["_authority"].as_str() == Some(&self.authority)
            && json["_clientId"].as_str() == Some(&self.client_id)
            && json["resource"].as_str() == Some(&self.resource)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::AccessTokenFileEntry;

    #[test]
    fn test_entry_parse() {
        let json = json!({
            "access_token": "KGVtcHR5KQ.eyJ0aWQiOiIxMjMiLCJ1bmlxdWVfbmFtZSI6InRlc3RAZXhhbXBsZS5jb20iLCJhcHBpZCI6IjEifQ.KGVtcHR5KQ",
            "resource": "r",
            "refresh_token": "KGVtcHR5KQ",
            "token_type": "Bearer",
            "expires_on": "1234567890"
        });
        let entry = AccessTokenFileEntry::parse(json).unwrap();
        assert_eq!("1", entry.client_id);
        assert_eq!("test@example.com", entry.user_id);
        assert_eq!("https://login.microsoftonline.com/123", entry.authority);
        assert_eq!("2009-02-13T23:31:30Z", entry.expires_on);
        assert_eq!("123", entry.tenant().unwrap());
    }
}
