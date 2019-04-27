use std::error::Error;

use reqwest;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
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

pub struct Client {
    access_token_file: Option<AccessTokenFile>,
    client: reqwest::Client,
}

impl Client {
    pub fn new() -> Client {
        return Client {
            access_token_file: AccessTokenFile::new().ok(),
            client: reqwest::Client::new(),
        };
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

        let (res, json) = self.request_json(request, &token)?;

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
                if let Some(entry) = self.refresh_token(request.resource, entry)? {
                    let (res, json) = self.request_json(request, &entry.access_token)?;
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

    fn get_value(&self, json: &Value) -> Result<Value> {
        if let Some(value) = json.get("value") {
            if !value.is_null() {
                return Ok(value.clone());
            }
        }
        return Ok(json.clone());
    }

    fn get_access_entry(&self, resource: &str) -> Result<AccessTokenFileEntry> {
        let file = self.access_token_file.as_ref().ok_or(HttpClientError)?;

        if let Some(entry) = file.find_entry(resource) {
            return Ok(entry);
        }

        if let Some(entry) = file.any_entry() {
            debug!("Trying to get access from existing refresh token...");
            if let Some(updated) = self.refresh_token(resource, &entry)? {
                return Ok(updated);
            }
        }

        return Err(HttpClientError.into());
    }

    fn refresh_token(
        &self,
        resource: &str,
        entry: &AccessTokenFileEntry,
    ) -> Result<Option<AccessTokenFileEntry>> {
        debug!("Refreshing token for {}", resource);

        let tenant = entry.tenant().ok_or(HttpClientError)?;
        let client_id = &entry.client_id;
        let refresh_token = &entry.refresh_token;

        let body = format!(
            "client_id={}&refresh_token={}&grant_type=refresh_token&resource={}",
            client_id, refresh_token, resource
        );

        let refresh_url = format!("https://login.microsoftonline.com/{}/oauth2/token", tenant);

        let mut res = self
            .client
            .post(refresh_url.as_str())
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body)
            .send()?;

        if res.status().is_success() {
            let json: Value = res.json()?;
            match json["access_token"].as_str() {
                Some(token) => {
                    let updated = AccessTokenFileEntry {
                        access_token: token.to_string(),
                        ..(entry.clone())
                    };

                    if let Some(file) = self.access_token_file.as_ref() {
                        file.update_entry(resource, &updated)?;
                    }

                    return Ok(Some(updated));
                }
                _ => (),
            }
        }

        return Ok(None);
    }

    fn request_json(&self, request: &Request, token: &str) -> Result<(Response, Value)> {
        let builder = match request.body {
            Some(body) => self.client.post(request.url).body(body.to_owned()),
            None => self.client.get(request.url),
        };

        let mut res = builder
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .header(CONTENT_TYPE, "application/json")
            .query(&[request.query])
            .send()?;

        trace!("Response: {:#?}", res);

        let json: Value = res.json()?;
        trace!("Response JSON: {:#?}", &json);

        if !res.status().is_success() {
            debug!("Request not successful: {}", res.status().as_str());
        }

        return Ok((res, json));
    }
}
