use dirs::home_dir;
use regex::Regex;
use serde_json::Value;
use url::Url;

use crate::error::AppError::InvalidAuthority;
use crate::error::AppError::InvalidIssuer;
use crate::error::AppError::InvalidTenantId;
use crate::error::AppError::UnexpectedJson;
use crate::utils::read_file;
use crate::utils::Result;

const AZURE_PROFILE_PATH: &'static str = ".azure/azureProfile.json";

#[derive(Clone, Debug)]
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

  pub fn from_name<F: FnOnce(&str) -> Result<Value>>(name: &str, request: F) -> Result<Tenant> {
    if Self::is_valid_id(name) {
      return Ok(Tenant {
        id: name.to_owned(),
      });
    }

    let url = format!(
      "https://login.microsoftonline.com/{}/.well-known/openid-configuration",
      name
    );
    let json = request(&url)?;

    let issuer = json
      .get("issuer")
      .and_then(Value::as_str)
      .ok_or_else(|| UnexpectedJson(json.clone()))?;

    let id = json
      .get("issuer")
      .and_then(Value::as_str)
      .and_then(|str| Url::parse(str).ok())
      .and_then(|url| {
        url
          .path_segments()
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
      let path = home_dir.join(AZURE_PROFILE_PATH);
      if let Some(subscriptions) = read_file(&path)?["subscriptions"].as_array() {
        for subscription in subscriptions {
          if subscription["isDefault"] == Value::Bool(true) {
            if let Some(id) = subscription["tenantId"].as_str() {
              debug!("Read default tenant from {}: {}", path.display(), id);
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
