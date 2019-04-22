use std::cell::RefCell;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use dirs::home_dir;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::from_reader;
use serde_json::from_value;
use serde_json::to_value;
use serde_json::Value;

use crate::error::AppError::AccessTokenFileError;

const ACCESS_TOKENS_PATH: &'static str = ".azure/accessTokens.json";

pub struct AccessTokenFile {
    path: Box<Path>,
    json: RefCell<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AccessTokenFileEntry {
    #[serde(rename = "_authority")]
    pub authority: String,

    #[serde(rename = "_clientId")]
    pub client_id: String,

    #[serde(rename = "accessToken")]
    pub access_token: String,

    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
}

impl AccessTokenFile {
    pub fn new() -> Result<AccessTokenFile, Box<Error>> {
        if let Some(home_dir) = home_dir() {
            let path = home_dir.join(ACCESS_TOKENS_PATH);
            if path.exists() {
                let file = File::open(&path)?;
                let reader = BufReader::new(file);
                let json = from_reader(reader)?;
                return Ok(AccessTokenFile {
                    path: path.into_boxed_path(),
                    json: RefCell::new(json),
                });
            }
        }
        return Err(AccessTokenFileError.into());
    }

    pub fn any_entry(&self) -> Option<AccessTokenFileEntry> {
        if let Some(arr) = self.json.borrow().as_array() {
            for entry in arr {
                if let Some(e) = from_value(entry.clone()).ok() {
                    return Some(e);
                }
            }
        }
        return None;
    }

    pub fn find_entry(&self, resource: &str) -> Option<AccessTokenFileEntry> {
        if let Some(arr) = self.json.borrow().as_array() {
            for entry in arr {
                if let Some(res) = entry["resource"].as_str() {
                    if res == resource {
                        return from_value(entry.clone()).ok();
                    }
                }
            }
        }
        return None;
    }

    pub fn update_entry(
        &self,
        resource: &str,
        entry: &AccessTokenFileEntry,
    ) -> Result<(), Box<Error>> {
        debug!("Updating access token file...");

        let mut json = self.json.try_borrow_mut()?;
        if let Some(arr) = json.as_array_mut() {
            let existing: Option<&mut Value> = arr.iter_mut().find(|e| {
                if let Some(res) = e["resource"].as_str() {
                    if res == resource {
                        return true;
                    }
                }
                return false;
            });

            match existing {
                Some(e) => entry.update_json(e)?,
                None => {
                    let mut value = to_value(entry)?;
                    value
                        .as_object_mut()
                        .unwrap()
                        .insert("resource".to_string(), Value::String(resource.to_string()));
                    arr.push(value);
                }
            }

            let file = File::create(&self.path)?;
            serde_json::to_writer(&file, arr)?;

            debug!("Updated token!");
        } else {
            debug!("JSON is not an array, skipping update!");
        }

        return Ok(());
    }
}

impl AccessTokenFileEntry {
    pub fn tenant(&self) -> Option<&str> {
        match self.authority.rfind("/") {
            Some(pos) => Some(&self.authority[pos..]),
            None => None,
        }
    }

    fn update_json(&self, json: &mut Value) -> Result<(), Box<Error>> {
        let map = json.as_object_mut().ok_or(AccessTokenFileError)?;
        map["accessToken"] = Value::String(self.access_token.clone());
        map["refreshToken"] = Value::String(self.refresh_token.clone());
        map["_clientId"] = Value::String(self.client_id.clone());
        map["_authority"] = Value::String(self.authority.clone());
        return Ok(());
    }
}
