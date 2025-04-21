use std::convert::TryInto;
use std::env::var_os;
use std::fs::create_dir_all;
use std::fs::File;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use chrono::DateTime;
use chrono::Local;
use chrono::LocalResult;
use chrono::NaiveDateTime;
use chrono::TimeZone;
use dirs::home_dir;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::from_slice;
use serde_json::from_value;
use serde_json::Value;

use crate::error::AppError::AccessTokenFileError;
use crate::error::AppError::InvalidAccessToken;
use crate::error::AppError::UnexpectedJson;
use crate::tenant::Tenant;
use crate::utils::read_file;
use crate::utils::Result;
use crate::utils::ValueExt;

const ACCESS_TOKENS_PATH: &'static str = ".azure/accessTokens.json";
const DEFAULT_EXPIRATION: u64 = 60 * 60 - 1;

#[derive(Clone, Debug)]
pub struct AccessToken {
    pub exp: i64,
    pub app_id: String,
    pub oid: String,
    pub unique_name: String,
    pub tenant: Tenant,
    token: String,
}

impl AccessToken {
    pub fn parse(token: String) -> Result<AccessToken> {
        let decoded = (|| -> Result<Value> {
            if let (Some(start), Some(end)) = (token.find('.'), token.rfind('.')) {
                let token = if start < end {
                    &token[start + 1..end]
                } else {
                    &token[start + 1..token.len()]
                };
                let decoded = base64::decode(token)?;
                return Ok(from_slice(decoded.as_slice())?);
            }
            return Err(InvalidAccessToken(token.to_owned()).into());
        })()?;

        let exp = decoded["exp"]
            .as_i64()
            .ok_or_else(|| UnexpectedJson(decoded.clone()))?;

        Ok(AccessToken {
            exp,
            app_id: decoded["appid"].string()?,
            oid: decoded["oid"].string()?,
            unique_name: decoded["unique_name"].string()?,
            tenant: Tenant::from_id(decoded["tid"].string()?)?,
            token,
        })
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn is_expired(&self) -> bool {
        since_unix_epoch(&SystemTime::now()) > self.exp
    }
}

#[derive(Clone, Debug)]
pub struct TokenSet {
    pub resource: String,
    pub access_token: AccessToken,
    pub refresh_token: String,
    pub expires_on: i64,
}

impl TokenSet {
    pub fn from_json(json: &Value) -> Result<TokenSet> {
        let access_token = AccessToken::parse(json["access_token"].string()?)?;

        let expires_on = if let Some(expires_on) = json["expires_on"].as_i64() {
            expires_on
        } else if let Some(expires_on) = json["expires_on"].as_str() {
            expires_on.parse::<i64>()?
        } else {
            return Err(UnexpectedJson(json.clone()).into());
        };

        if access_token.exp != expires_on {
            debug!("Different exp: {:?} != {:?}", access_token.exp, expires_on);
        }

        Ok(TokenSet {
            resource: json["resource"].string()?,
            access_token,
            refresh_token: json["refresh_token"].string()?,
            expires_on,
        })
    }

    pub fn find(
        token_sets: &Vec<TokenSet>,
        client_id: &str,
        authority: &str,
        resource: Option<&str>,
    ) -> Option<TokenSet> {
        token_sets
            .iter()
            .find(|token_set| {
                token_set.access_token.app_id == client_id
                    && token_set.access_token.tenant.authority() == authority
                    && (resource == None || token_set.resource == resource.unwrap())
            })
            .map(|token_set| token_set.clone())
            .or_else(|| {
                debug!(
                    "Did not find token set: {} {} {:?}",
                    client_id, authority, resource
                );
                None
            })
    }

    pub fn expires_on(&self) -> String {
        match Local.timestamp_opt(self.expires_on, 0) {
            LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S%.6f").to_string(),
            _ => "1970-01-01 00:00:00.000000".to_owned(),
        }
    }

    pub fn matches(&self, token_set: &TokenSet) -> bool {
        token_set.access_token.tenant == self.access_token.tenant
            && token_set.resource == self.resource
            && token_set.access_token.app_id == self.access_token.app_id
            && token_set.access_token.unique_name == self.access_token.unique_name
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccessTokenFileEntry {
    token_type: String,

    expires_in: u64,
    expires_on: String,

    resource: String,

    access_token: String,
    refresh_token: String,

    oid: Option<String>,
    user_id: Option<String>,

    #[serde(rename = "isMRRT")]
    multiple_resource_refresh_token: Option<bool>,

    #[serde(rename = "_clientId")]
    client_id: String,

    #[serde(rename = "_authority")]
    authority: String,
}

impl AccessTokenFileEntry {
    fn from_token_set(token_set: &TokenSet) -> Result<AccessTokenFileEntry> {
        Ok(AccessTokenFileEntry {
            token_type: "Bearer".to_owned(),
            expires_in: DEFAULT_EXPIRATION,
            expires_on: token_set.expires_on(),
            resource: token_set.resource.clone(),
            access_token: token_set.access_token.token.clone(),
            refresh_token: token_set.refresh_token.clone(),
            oid: Some(token_set.access_token.oid.clone()),
            user_id: Some(token_set.access_token.unique_name.clone()),
            multiple_resource_refresh_token: Some(true),
            client_id: token_set.access_token.app_id.clone(),
            authority: token_set.access_token.tenant.authority(),
        })
    }

    fn to_token_set(&self) -> Result<TokenSet> {
        let access_token = AccessToken::parse(self.access_token.clone())?;
        let tenant = Tenant::from_authority(&self.authority)?;
        if access_token.tenant.id != tenant.id {
            debug!(
                "Mismatched tenant ID: {} != {}",
                access_token.tenant.id, tenant.id
            );
        }
        let expires_on =
            match NaiveDateTime::parse_from_str(&self.expires_on, "%Y-%m-%d %H:%M:%S%.6f") {
                Ok(date) => date.and_local_timezone(Local).unwrap().timestamp(),
                Err(_) => DateTime::parse_from_rfc3339(&self.expires_on)?.timestamp(),
            };
        Ok(TokenSet {
            resource: self.resource.clone(),
            access_token,
            refresh_token: self.refresh_token.clone(),
            expires_on,
        })
    }

    fn matches(&self, token_set: &TokenSet) -> bool {
        &self.token_type == "Bearer"
            && token_set.access_token.tenant.authority() == self.authority
            && token_set.resource == self.resource
            && token_set.access_token.app_id == self.client_id
            && Some(&token_set.access_token.unique_name) == self.user_id.as_ref()
    }

    fn update_from(&mut self, token_set: &TokenSet) {
        self.access_token = token_set.access_token.token().to_owned();
        self.refresh_token = token_set.refresh_token.clone();
        self.expires_on = token_set.expires_on();
    }
}

fn since_unix_epoch(time: &SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().try_into().unwrap_or(0),
        Err(_) => 0,
    }
}

pub struct AccessTokenFile {
    path: PathBuf,
}

impl AccessTokenFile {
    pub fn new() -> Result<AccessTokenFile> {
        let path = if let Some(ref path) = var_os("AZURE_ACCESS_TOKEN_FILE") {
            PathBuf::from(path)
        } else if let Some(ref home_dir) = home_dir() {
            home_dir.join(ACCESS_TOKENS_PATH)
        } else {
            return Err(AccessTokenFileError.into());
        };
        Ok(AccessTokenFile { path })
    }

    pub fn read_tokens(&self) -> Result<Vec<TokenSet>> {
        Ok(self
            .read_entries()?
            .into_iter()
            .map(|entry| Ok(entry.to_token_set()?))
            .collect::<Result<Vec<TokenSet>>>()?)
    }

    fn read_entries(&self) -> Result<Vec<AccessTokenFileEntry>> {
        trace!("Reading accessTokens.json from {}", self.path.display());
        if let Some(arr) = read_file(&self.path)?.as_array() {
            let entries = arr
                .into_iter()
                .map(|json| Ok(from_value(json.clone())?))
                .collect::<Result<Vec<AccessTokenFileEntry>>>()?;
            trace!("Read access token entries: {:#?}", entries);
            Ok(entries)
        } else {
            Ok(vec![])
        }
    }

    pub fn update_tokens(&self, token_sets: &Vec<TokenSet>) -> Result<()> {
        let mut entries = self.read_entries()?;

        for token_set in token_sets {
            let mut updated = false;
            for e in entries.iter_mut() {
                if e.matches(token_set) {
                    e.update_from(token_set);
                    updated = true;
                    trace!("Updated token: {:?}", e);
                }
            }

            if !updated {
                entries.push(AccessTokenFileEntry::from_token_set(token_set)?);
                trace!("Added new token: {:?}", token_set);
            }
        }

        if let Some(parent) = self.path.parent() {
            create_dir_all(parent)?;
        }

        let file = File::create(&self.path)?;
        serde_json::to_writer(&file, &entries)?;
        debug!("Written access token file: {}", self.path.display());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use chrono::Local;
    use serde_json::from_value;
    use serde_json::json;

    use super::AccessTokenFileEntry;
    use super::TokenSet;

    const AT: &str = "eyJhbGciOiJub25lIn0.eyJleHAiOjEsInRpZCI6IjEyMzQ1Njc4LTEyMzQtMTIzNC0xMjM0LWFiY2RlZjEyMzQ1NiIsInVuaXF1ZV9uYW1lIjoidGVzdEBleGFtcGxlLmNvbSIsImFwcGlkIjoiMSIsIm9pZCI6IjEyMyJ9";

    #[test]
    fn test_token_set_parse() {
        let json = json!({
            "access_token": AT,
            "resource": "r",
            "refresh_token": "0",
            "token_type": "Bearer",
            "expires_on": "1234567890"
        });
        let token_set = TokenSet::from_json(&json).unwrap();
        assert_eq!("1", token_set.access_token.app_id);
        assert_eq!("123", token_set.access_token.oid);
        assert_eq!("0", token_set.refresh_token);
        assert_eq!("test@example.com", token_set.access_token.unique_name);
        assert_eq!(
            "12345678-1234-1234-1234-abcdef123456",
            token_set.access_token.tenant.id
        );
        assert_eq!(to_date("2009-02-13T23:31:30Z"), token_set.expires_on());
    }

    #[test]
    fn test_entry_parse() {
        let expires_on = to_date("2020-02-02T20:02:20Z");
        let json = json!({
            "tokenType": "Bearer",
            "expiresIn": 3599,
            "expiresOn": expires_on,
            "resource": "r",
            "accessToken": AT,
            "refreshToken": "0",
            "_clientId": "123",
            "_authority": "https://login.microsoftonline.com/12345678-1234-1234-1234-abcdef123456"
        });
        let entry: AccessTokenFileEntry = from_value(json).unwrap();
        let token_set = entry.to_token_set().unwrap();
        assert_eq!("1", token_set.access_token.app_id);
        assert_eq!("123", token_set.access_token.oid);
        assert_eq!("test@example.com", token_set.access_token.unique_name);
        assert_eq!(
            "12345678-1234-1234-1234-abcdef123456",
            token_set.access_token.tenant.id
        );
        assert_eq!(expires_on, token_set.expires_on());
    }

    fn to_date(s: &str) -> String {
        DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S%.6f")
            .to_string()
    }
}
