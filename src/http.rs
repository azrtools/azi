use rustls::ClientConfig;
use serde_json::from_reader;
use serde_json::to_string_pretty;
use serde_json::Value;
use std::sync::Arc;
use ureq::Agent;
use ureq::AgentBuilder;

use crate::error::AppError::HttpClientError;
use crate::error::AppError::HttpError;
use crate::error::AppError::InvalidCertificate;
use crate::utils::Result;

#[derive(Debug)]
pub struct Header {
  name: &'static str,
  value: String,
}

impl Header {
  pub fn new(name: &'static str, value: String) -> Self {
    Self { name, value }
  }

  pub fn content_form() -> Self {
    Self::new(
      "Content-Type",
      "application/x-www-form-urlencoded".to_owned(),
    )
  }

  pub fn content_json() -> Self {
    Self::new("Content-Type", "application/json".to_owned())
  }

  pub fn auth_bearer(token: &str) -> Self {
    Self::new("Authorization", format!("Bearer {}", token))
  }
}

pub struct Http {
  agent: Agent,
  url: Option<String>,
  headers: Option<Vec<Header>>,
}

impl Http {
  pub fn new() -> Self {
    Self::for_agent(AgentBuilder::new().build())
  }

  pub fn for_certificate_authority(ca: &str) -> Result<Self> {
    let mut client_config = ClientConfig::new();
    match client_config.root_store.add_pem_file(&mut ca.as_bytes()) {
      Ok(_) => (),
      Err(_) => return Err(InvalidCertificate(ca.to_owned()).into()),
    };
    Ok(Self::for_agent(
      AgentBuilder::new()
        .tls_config(Arc::new(client_config))
        .build(),
    ))
  }

  pub fn for_agent(agent: Agent) -> Self {
    Http {
      agent,
      url: None,
      headers: None,
    }
  }

  pub fn with_url(self, url: String) -> Self {
    Http {
      agent: self.agent,
      url: Some(url),
      headers: self.headers,
    }
  }

  pub fn with_headers(self, headers: Vec<Header>) -> Self {
    Http {
      agent: self.agent,
      url: self.url,
      headers: Some(headers),
    }
  }

  pub fn get(&self, url: &str) -> Result<Response> {
    self.execute(url, None, Option::None)
  }

  pub fn post(&self, url: &str, body: &str) -> Result<Response> {
    self.execute(url, None, Some(body))
  }

  pub fn execute(
    &self,
    url: &str,
    headers: Option<&Vec<Header>>,
    body: Option<&str>,
  ) -> Result<Response> {
    let url = match &self.url {
      Some(base) => format!("{}{}", base, url),
      None => url.to_owned(),
    };

    debug!("Requesting: {}", url);

    trace!("Request headers: {:?}", &headers);
    trace!("Request body: {:?}", &body);

    if url.starts_with("http://") {
      warn!("Plain HTTP requested!");
      return Err(HttpClientError.into());
    }

    let mut request = if body.is_some() {
      self.agent.post(&url)
    } else {
      self.agent.get(&url)
    };

    if let Some(headers) = &self.headers {
      for header in headers {
        request = request.set(header.name, &header.value);
      }
    }

    if let Some(headers) = headers {
      for header in headers {
        request = request.set(header.name, &header.value);
      }
    }

    let result = if let Some(body) = body {
      request.send_string(body)
    } else {
      request.call()
    };

    match result {
      Ok(response) => {
        trace!("Response: {}", response.status());
        Ok(Response::Success(to_json(response)))
      }
      Err(ureq::Error::Status(status, response)) => {
        debug!("Request not successful: {}", status);
        Ok(Response::Error(status, to_json(response)))
      }
      Err(err) => {
        debug!("Request failed!");
        Err(err.into())
      }
    }
  }
}

pub enum Response {
  Success(Value),
  Error(u16, Value),
}

impl Response {
  pub fn success(self) -> Result<Value> {
    match self {
      Response::Success(json) => Ok(json),
      Response::Error(status, json) => Err(HttpError(status, json).into()),
    }
  }
}

fn to_json(response: ureq::Response) -> Value {
  match from_reader::<_, Value>(response.into_reader()) {
    Ok(json) => {
      match to_string_pretty(&json) {
        Ok(s) => debug!("Response JSON: {}", s),
        Err(_) => debug!("Response JSON: {:?}", json),
      }
      json
    }
    Err(err) => {
      debug!("Response JSON could not be parsed: {}", err);
      Value::Null
    }
  }
}
