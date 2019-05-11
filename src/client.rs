use std::error::Error;
use std::thread::sleep;
use std::time::Duration;

use reqwest;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::RequestBuilder;
use reqwest::Response;
use serde::de::DeserializeOwned;
use serde_json::from_value;
use serde_json::Value;

use crate::auth::AccessTokenFile;
use crate::auth::AccessTokenFileEntry;
use crate::error::AppError::HttpClientError;

type Result<T> = std::result::Result<T, Box<Error>>;

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
    client: reqwest::Client,
}

impl Client {
    pub fn new(tenant: Option<&str>) -> Result<Client> {
        return Ok(Client {
            tenant: tenant.map(str::to_string),
            access_token_file: AccessTokenFile::new(tenant)?,
            client: reqwest::Client::new(),
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
        debug!("Requesting: {}", request.url);

        let entry = self.get_access_entry(request.resource)?;
        let token: &String = &entry.access_token;

        let (res, json) = self.execute_request(request, &token)?;

        if res.status().is_success() {
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
            if code == "ExpiredAuthenticationToken" {
                debug!("Auth token expired!");
                if let Some(entry) = self.refresh_token(&entry.resource, entry)? {
                    let (res, json) = self.execute_request(request, &entry.access_token)?;
                    if res.status().is_success() {
                        return self.get_value(&json);
                    }
                }
            } else {
                debug!("Unknown error: {}", code);
            }
        }
        return Err(HttpClientError.into());
    }

    fn execute_request(&self, request: &Request, token: &str) -> Result<(Response, Value)> {
        let builder = match request.body {
            Some(body) => self.client.post(request.url).body(body.to_owned()),
            None => self.client.get(request.url),
        };

        return self.request_json(
            builder
                .header(AUTHORIZATION, format!("Bearer {}", token))
                .header(CONTENT_TYPE, "application/json")
                .query(&[request.query]),
        );
    }

    fn get_value(&self, json: &Value) -> Result<Value> {
        let value = &json["value"];
        if value.is_null() {
            return Ok(json.clone());
        } else {
            return Ok(value.clone());
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
        let (res, json) = self.request_json(
            self.client
                .post(refresh_url.as_str())
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(body),
        )?;

        if res.status().is_success() {
            if let Some(token) = json["access_token"].as_str() {
                let updated = entry.with_token(token.to_string(), resource.to_string());
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

        let (res, json) = self.request_json(
            self.client
                .post(url.as_str())
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(body),
        )?;

        if res.status().is_success() {
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

                let (res, json) = self.request_json(
                    self.client
                        .post(url.as_str())
                        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                        .body(body),
                )?;

                if res.status().is_success() {
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

    fn request_json(&self, request: RequestBuilder) -> Result<(Response, Value)> {
        let mut res = request.send()?;

        trace!("Response: {:#?}", res);

        if !res.status().is_success() {
            debug!("Request not successful: {}", res.status().as_str());
        }

        let json: Value = match res.json() {
            Ok(json) => {
                trace!("Response JSON: {:#?}", &json);
                json
            }
            Err(err) => {
                trace!("Response JSON could not be parsed: {}", err);
                Value::Null
            }
        };

        return Ok((res, json));
    }
}
