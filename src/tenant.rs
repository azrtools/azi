use dirs::home_dir;
use regex::Regex;
use serde_json::Value;
use url::Url;

use crate::error::AppError::InvalidAuthority;
use crate::error::AppError::InvalidIssuer;
use crate::error::AppError::InvalidTenantId;
use crate::error::AppError::UnexpectedJson;
use crate::http::Http;
use crate::utils::read_file;
use crate::utils::Result;

const AZURE_PROFILE_PATH: &'static str = ".azure/azureProfile.json";
const ACCESS_TOKENS_PATH: &'static str = ".azure/accessTokens.json";

#[derive(Clone, Debug, PartialEq)]
pub struct Tenant {
    pub id: String,
}

impl Tenant {
    pub fn common() -> Tenant {
        Tenant {
            id: "common".to_owned(),
        }
    }

    pub fn from_id(id: String) -> Result<Tenant> {
        match Self::is_valid_id(&id) {
            true => Ok(Tenant { id }),
            false => Err(InvalidTenantId(id).into()),
        }
    }

    pub fn from_authority(authority: &str) -> Result<Tenant> {
        match authority.rfind("/") {
            Some(pos) => Self::from_id(authority[pos + 1..].to_owned()),
            None => Err(InvalidAuthority(authority.to_owned()).into()),
        }
    }

    pub fn from_name(name: &str, http: &Http) -> Result<Tenant> {
        if Self::is_valid_id(name) {
            return Ok(Tenant {
                id: name.to_owned(),
            });
        }

        let url = format!(
            "https://login.microsoftonline.com/{}/.well-known/openid-configuration",
            name
        );

        let json = http.execute(&url, None, None)?.success()?;

        let issuer = json
            .get("issuer")
            .and_then(Value::as_str)
            .ok_or_else(|| UnexpectedJson(json.clone()))?;

        let id = json
            .get("issuer")
            .and_then(Value::as_str)
            .and_then(|str| Url::parse(str).ok())
            .and_then(|url| {
                url.path_segments()
                    .and_then(|mut segments| segments.next())
                    .map(|s| s.to_owned())
            })
            .ok_or(InvalidIssuer(issuer.to_owned()))?;

        match Self::is_valid_id(&id) {
            true => Ok(Tenant { id }),
            false => Err(InvalidIssuer(issuer.to_owned()).into()),
        }
    }

    fn is_valid_id(id: &str) -> bool {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"^[0-9a-fA-F-]{36}$").unwrap();
        }
        id == "common" || RE.is_match(id)
    }

    pub fn read_default_tenant() -> Result<Option<Tenant>> {
        if let Some(ref home_dir) = home_dir() {
            let profile = home_dir.join(AZURE_PROFILE_PATH);
            if let Some(subscriptions) = read_file(&profile)?["subscriptions"].as_array() {
                for subscription in subscriptions {
                    if subscription["isDefault"] == Value::Bool(true) {
                        if let Some(id) = subscription["tenantId"].as_str() {
                            debug!("Read default tenant from {}: {}", profile.display(), id);
                            return Ok(Some(Tenant { id: id.to_owned() }));
                        }
                    }
                }
            }

            let tokens = home_dir.join(ACCESS_TOKENS_PATH);
            if let Some(entries) = read_file(&tokens)?.as_array() {
                if entries.len() == 1 {
                    if let Some(authority) = entries[0]["_authority"].as_str() {
                        if authority.len() == 70
                            && authority.starts_with("https://login.microsoftonline.com/")
                        {
                            let id = &authority[34..];
                            debug!("Read default tenant from {}: {}", tokens.display(), id);
                            return Ok(Some(Tenant { id: id.to_owned() }));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    pub fn is_common(&self) -> bool {
        self.id == "common"
    }

    pub fn authority(&self) -> String {
        format!("https://login.microsoftonline.com/{}", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::Tenant;

    #[test]
    fn test_is_valid_id() {
        assert_eq!(
            true,
            Tenant::is_valid_id("12345678-1234-1234-1234-abcdef123456")
        );
        assert_eq!(true, Tenant::is_valid_id("common"));
        assert_eq!(false, Tenant::is_valid_id(""));
    }
}
