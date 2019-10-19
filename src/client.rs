use std::cell::RefCell;
use std::io::Read;
use std::mem::drop;
use std::thread::sleep;
use std::time::Duration;

use curl::easy::Easy;
use curl::easy::List;
use serde::de::DeserializeOwned;
use serde_json::from_slice;
use serde_json::from_value;
use serde_json::Value;
use url::Url;

use crate::auth::AccessTokenFile;
use crate::auth::AccessTokenFileEntry;
use crate::error::AppError::HttpClientError;
use crate::utils::Result;

pub struct Request<'r> {
    client: &'r Client,
    url: &'r str,
    resource: &'r str,
    query: (&'r str, &'r str),
    body: Option<&'r str>,
}

impl<'r> Request<'r> {
    pub fn query(mut self, name: &'r str, value: &'r str) -> Self {
        self.query = (name, value);
        return self;
    }

    pub fn body(mut self, body: &'r str) -> Self {
        self.body = Some(body);
        return self;
    }

    pub fn get_raw(&self) -> Result<Value> {
        return self.client.request(self);
    }

    pub fn post(&mut self) -> Result<Value> {
        return self.client.request(self);
    }

    pub fn get_list<T>(&self) -> Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let json = self.client.request(self)?;
        if let Some(arr) = json.as_array() {
            let mut vec = Vec::new();
            for entry in arr {
                let item: T = from_value(entry.clone())?;
                vec.push(item);
            }
            return Ok(vec);
        }

        debug!("Response is not a JSON array!");
        return Err(HttpClientError.into());
    }
}

const CLIENT_ID: &'static str = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";

pub struct Client {
    tenant: Option<String>,
    access_token_file: AccessTokenFile,
    handle: RefCell<Easy>,
}

impl Client {
    pub fn new(tenant: Option<&str>) -> Result<Client> {
        let mut handle = Easy::new();
        handle.useragent("github.com/pascalgn/azi")?;
        return Ok(Client {
            tenant: tenant.map(str::to_owned),
            access_token_file: AccessTokenFile::new(tenant)?,
            handle: RefCell::new(handle),
        });
    }

    pub fn new_request<'c>(&'c self, url: &'c str, resource: &'c str) -> Request<'c> {
        return Request {
            client: &self,
            url,
            resource,
            query: ("", ""),
            body: None,
        };
    }

    fn request(&self, request: &Request) -> Result<Value> {
        let entry = self.get_access_entry(request.resource)?;
        let token: &String = &entry.access_token;

        let (status, json) = self.execute_request(request, &token)?;

        if status.is_success() {
            return self.get_value(&json);
        } else {
            return self.try_rerequest(&entry, request, &json);
        }
    }

    fn try_rerequest(
        &self,
        entry: &AccessTokenFileEntry,
        request: &Request,
        json: &Value,
    ) -> Result<Value> {
        if let Some(code) = json["error"]["code"].as_str() {
            if code == "ExpiredAuthenticationToken" || code == "AuthenticationFailed" {
                debug!("Auth token expired!");
                if let Some(entry) = self.refresh_token(&entry.resource, entry)? {
                    let (status, json) = self.execute_request(request, &entry.access_token)?;
                    if status.is_success() {
                        return self.get_value(&json);
                    }
                }
            } else {
                debug!("Unknown error: {}", code);
            }
        }
        Err(HttpClientError.into())
    }

    fn execute_request(&self, request: &Request, token: &str) -> Result<(Status, Value)> {
        let (key, value) = request.query;
        let url = if key.len() > 0 && value.len() > 0 {
            let mut url = Url::parse(request.url)?;
            url.query_pairs_mut().append_pair(key, value);
            url.to_string()
        } else {
            request.url.to_owned()
        };
        self.execute_raw(&url, Self::headers_json(token)?, request.body)
    }

    fn get_value(&self, json: &Value) -> Result<Value> {
        let value = &json["value"];
        if value.is_null() {
            Ok(json.clone())
        } else {
            Ok(value.clone())
        }
    }

    fn get_access_entry(&self, resource: &str) -> Result<AccessTokenFileEntry> {
        if let Some(entry) = self.access_token_file.find_entry(resource) {
            return Ok(entry.clone());
        }

        if let Some(entry) = self.access_token_file.any_entry() {
            debug!("Trying to get access from existing refresh token...");
            if let Some(updated) = self.refresh_token(resource, &entry)? {
                return Ok(updated);
            }
        }

        debug!("Trying to get new access token...");
        return self.request_new_token(resource);
    }

    fn refresh_token(
        &self,
        resource: &str,
        entry: &AccessTokenFileEntry,
    ) -> Result<Option<AccessTokenFileEntry>> {
        debug!("Refreshing token for {}", resource);

        trace!("Current token: {}", entry.access_token);

        let tenant = entry.tenant().ok_or(HttpClientError)?;
        let client_id = &entry.client_id;
        let refresh_token = &entry.refresh_token;

        let body = format!(
            "client_id={}&refresh_token={}&grant_type=refresh_token&resource={}",
            client_id, refresh_token, resource
        );

        let refresh_url = format!("https://login.microsoftonline.com/{}/oauth2/token", tenant);
        let (status, json) = self.execute_raw(&refresh_url, Self::headers_form()?, Some(&body))?;

        if status.is_success() {
            if let Some(token) = json["access_token"].as_str() {
                let updated = entry.with_token(token.to_owned(), resource.to_owned());
                self.access_token_file.update_entry(&updated)?;
                return Ok(Some(updated));
            }
        } else if json["error"].as_str() == Some("invalid_grant") {
            debug!("Refresh token is no longer valid!");
            return self.request_new_token(resource).map(Some);
        }

        return Ok(None);
    }

    fn request_new_token(&self, resource: &str) -> Result<AccessTokenFileEntry> {
        let tenant = self.tenant.as_ref().map(String::as_str).unwrap_or("common");
        let url = format!(
            "https://login.microsoftonline.com/{}/oauth2/devicecode?api-version=1.0",
            tenant
        );

        let body = format!("client_id={}&resource={}", CLIENT_ID, resource);

        let (status, json) = self.execute_raw(&url, Self::headers_form()?, Some(&body))?;

        if status.is_success() {
            let device_code = json["device_code"].as_str().ok_or(HttpClientError)?;

            let message = json["message"].as_str().ok_or(HttpClientError)?;
            println!("{}", message);

            loop {
                sleep(Duration::from_millis(5000));

                let url = format!("https://login.microsoftonline.com/{}/oauth2/token", tenant);
                let body = format!(
                    "grant_type=device_code&client_id={}&resource={}&code={}",
                    CLIENT_ID, resource, device_code
                );

                let (status, json) = self.execute_raw(&url, Self::headers_form()?, Some(&body))?;

                if status.is_success() {
                    let entry = AccessTokenFileEntry::parse(json)?;
                    self.access_token_file.update_entry(&entry)?;

                    if let Some(tenant) = self.tenant.as_ref() {
                        // the tenant recorded in the file is just the ID, but the tenant given
                        // via --tenant flag is probably given as name, so make sure we also
                        // record an entry for the tenant name
                        let for_tenant = entry.with_tenant(tenant);
                        self.access_token_file.update_entry(&for_tenant)?;
                    }

                    return Ok(entry);
                } else if json["error"].as_str() == Some("authorization_pending") {
                    debug!("Authorization pending...");
                } else {
                    warn!("Unknown error response: {}", json);
                    break;
                }
            }
        }

        return Err(HttpClientError.into());
    }

    fn headers_form() -> Result<List> {
        let mut headers = List::new();
        headers.append("Content-Type: application/x-www-form-urlencoded")?;
        return Ok(headers);
    }

    fn headers_json(token: &str) -> Result<List> {
        let mut headers = List::new();
        headers.append(format!("Authorization: Bearer {}", token).as_str())?;
        headers.append("Content-Type: application/json")?;
        return Ok(headers);
    }

    fn execute_raw(&self, url: &str, headers: List, body: Option<&str>) -> Result<(Status, Value)> {
        debug!("Requesting: {}", url);

        trace!("Request body: {:#?}", &body);

        let mut handle = self.handle.try_borrow_mut()?;

        handle.url(url)?;
        handle.http_headers(headers)?;

        let mut data = body.unwrap_or("").as_bytes();

        if body.is_some() {
            handle.post(true)?;
            handle.post_field_size(data.len() as u64)?;
        } else {
            handle.get(true)?;
        }

        let mut response: Vec<u8> = vec![];
        let mut transfer = handle.transfer();
        transfer.read_function(|buf| Ok(data.read(buf).unwrap_or(0)))?;
        transfer.write_function(|buf| {
            response.extend(buf);
            Ok(buf.len())
        })?;

        if let Err(err) = transfer.perform() {
            debug!("Request failed!");
            return Err(err.into());
        }

        drop(transfer);

        let status = handle.response_code()?;

        trace!("Response: {}", status);

        if !status.is_success() {
            debug!("Request not successful: {}", status);
        }

        let json: Value = match from_slice(response.as_slice()) {
            Ok(json) => {
                trace!("Response JSON: {:#?}", &json);
                json
            }
            Err(err) => {
                trace!("Response JSON could not be parsed: {}", err);
                Value::Null
            }
        };

        return Ok((status, json));
    }
}

type Status = u32;

trait StatusExt {
    fn is_success(&self) -> bool;
}

impl StatusExt for Status {
    fn is_success(&self) -> bool {
        return *self >= 200 && *self < 400;
    }
}
