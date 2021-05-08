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
use crate::auth::TokenSet;
use crate::error::AppError::HttpClientError;
use crate::error::AppError::UnexpectedJson;
use crate::tenant::Tenant;
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
    tenant: RefCell<Tenant>,
    access_token_file: AccessTokenFile,
    token_sets: RefCell<Vec<TokenSet>>,
    handle: RefCell<Easy>,
}

impl Client {
    pub fn new(tenant: Option<&str>) -> Result<Client> {
        let mut handle = Easy::new();
        handle.useragent("github.com/pascalgn/azi")?;

        let tenant = match tenant {
            Some(tenant) => Tenant::from_name(tenant, |url| {
                execute(&mut handle, url, List::new(), None).map(|res| res.1)
            })?,
            None => Tenant::read_default_tenant()?.unwrap_or(Tenant::common()),
        };

        let access_token_file = AccessTokenFile::new()?;
        let token_sets = access_token_file.read_tokens()?;

        debug!("Client created with tenant: {}", tenant.id);

        Ok(Client {
            tenant: RefCell::new(tenant),
            access_token_file,
            token_sets: RefCell::new(token_sets),
            handle: RefCell::new(handle),
        })
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
        let token_set = self.get_token_set(request.resource)?;

        let (status, json) = self.execute_request(request, &token_set)?;

        if status.is_success() {
            return self.get_value(&json);
        } else {
            return self.try_rerequest(&token_set, request, &json);
        }
    }

    fn try_rerequest(
        &self,
        token_set: &TokenSet,
        request: &Request,
        json: &Value,
    ) -> Result<Value> {
        if let Some(code) = json["error"]["code"].as_str() {
            if code == "ExpiredAuthenticationToken" || code == "AuthenticationFailed" {
                debug!("Auth token expired!");
                let token_set = self.refresh_token(request.resource, token_set)?;
                let (status, json) = self.execute_request(request, &token_set)?;
                if status.is_success() {
                    return self.get_value(&json);
                } else {
                    return Err(HttpClientError.into());
                }
            } else {
                debug!("Unknown error: {}", code);
            }
        }
        Err(UnexpectedJson(json.clone()).into())
    }

    fn execute_request(&self, request: &Request, tokens: &TokenSet) -> Result<(Status, Value)> {
        let (key, value) = request.query;
        let url = if key.len() > 0 && value.len() > 0 {
            let mut url = Url::parse(request.url)?;
            url.query_pairs_mut().append_pair(key, value);
            url.to_string()
        } else {
            request.url.to_owned()
        };
        let access_token = tokens.access_token.token();
        self.execute_raw(&url, Self::headers_json(access_token)?, request.body)
    }

    fn get_value(&self, json: &Value) -> Result<Value> {
        let value = &json["value"];
        if value.is_null() {
            Ok(json.clone())
        } else {
            Ok(value.clone())
        }
    }

    fn get_token_set(&self, resource: &str) -> Result<TokenSet> {
        let authority = {
            let tenant = self.tenant.try_borrow()?;
            tenant.authority()
        };

        if let Some(token_set) = {
            let token_sets = self.token_sets.try_borrow()?;
            TokenSet::find(&token_sets, &authority, resource)
        } {
            if token_set.access_token.is_expired() {
                trace!("Found expired token set: {:?}", token_set);
                return Ok(self.refresh_token(resource, &token_set)?);
            } else {
                trace!("Found valid token set: {:?}", token_set);
                return Ok(token_set.clone());
            }
        }

        if let Some(token_set) = {
            let token_sets = self.token_sets.try_borrow()?;
            TokenSet::find_any(&token_sets, &authority)
        } {
            debug!("Trying to get access from existing refresh token...");
            return Ok(self.refresh_token(resource, &token_set)?);
        }

        debug!("Trying to get new access token...");
        return self.request_new_token(resource);
    }

    fn refresh_token(&self, resource: &str, token_set: &TokenSet) -> Result<TokenSet> {
        debug!("Refreshing token for {}", resource);

        trace!("Current token: {:?}", token_set);

        let tenant_id = {
            let tenant = self.tenant.try_borrow()?;
            tenant.id.clone()
        };

        let body = format!(
            "client_id={}&refresh_token={}&grant_type=refresh_token&resource={}",
            CLIENT_ID, token_set.refresh_token, resource
        );
        let refresh_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/token",
            tenant_id
        );
        let (status, json) = self.execute_raw(&refresh_url, Self::headers_form()?, Some(&body))?;

        if status.is_success() {
            let token_set = TokenSet::from_json(&json)?;
            self.update_tokens(&token_set)?;
            return Ok(token_set);
        } else if json["error"].as_str() == Some("invalid_grant") {
            debug!("Refresh token is no longer valid!");
            return Ok(self.request_new_token(resource)?);
        } else {
            return Err(UnexpectedJson(json).into());
        }
    }

    fn request_new_token(&self, resource: &str) -> Result<TokenSet> {
        let tenant = self.tenant.try_borrow()?;

        let url = format!(
            "https://login.microsoftonline.com/{}/oauth2/devicecode?api-version=1.0",
            tenant.id
        );

        let body = format!("client_id={}&resource={}", CLIENT_ID, resource);

        let (status, json) = self.execute_raw(&url, Self::headers_form()?, Some(&body))?;

        if status.is_success() {
            let device_code = json["device_code"].as_str().ok_or(HttpClientError)?;

            let message = json["message"].as_str().ok_or(HttpClientError)?;
            eprintln!("{}", message);

            loop {
                sleep(Duration::from_millis(5000));

                let url = format!(
                    "https://login.microsoftonline.com/{}/oauth2/token",
                    tenant.id
                );
                let body = format!(
                    "grant_type=device_code&client_id={}&resource={}&code={}",
                    CLIENT_ID, resource, device_code
                );
                let (status, json) = self.execute_raw(&url, Self::headers_form()?, Some(&body))?;

                if status.is_success() {
                    let token_set = TokenSet::from_json(&json)?;
                    self.update_tokens(&token_set)?;

                    if tenant.is_common() {
                        drop(tenant);
                        self.tenant.replace(token_set.access_token.tenant.clone());
                    }

                    return Ok(token_set);
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

    fn update_tokens(&self, token_set: &TokenSet) -> Result<()> {
        let mut token_sets = { self.token_sets.try_borrow()?.clone() };
        let mut updated = false;
        for mut t in token_sets.iter_mut() {
            if t.matches(token_set) {
                t.access_token = token_set.access_token.clone();
                t.refresh_token = token_set.refresh_token.clone();
                t.expires_on = token_set.expires_on.clone();
                updated = true;
            }
        }
        if !updated {
            token_sets.push(token_set.clone());
        }
        self.access_token_file.update_tokens(&token_sets)?;
        self.token_sets.replace(token_sets);
        Ok(())
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
        let mut handle = self.handle.try_borrow_mut()?;
        return execute(&mut handle, url, headers, body);
    }
}

fn execute(
    handle: &mut Easy,
    url: &str,
    headers: List,
    body: Option<&str>,
) -> Result<(Status, Value)> {
    debug!("Requesting: {}", url);

    trace!("Request body: {:#?}", &body);

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

type Status = u32;

trait StatusExt {
    fn is_success(&self) -> bool;
}

impl StatusExt for Status {
    fn is_success(&self) -> bool {
        return *self >= 200 && *self < 400;
    }
}
