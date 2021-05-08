use serde_json::from_reader;
use serde_json::to_string_pretty;
use serde_json::Value;
use ureq::Agent;
use ureq::AgentBuilder;

use crate::error::AppError::HttpClientError;
use crate::utils::Result;

pub type Header = (&'static str, String);

pub struct Http {
  agent: Agent,
}

impl Http {
  pub fn new() -> Result<Http> {
    Ok(Http {
      agent: AgentBuilder::new().build(),
    })
  }

  pub fn get(&self, url: &str) -> Result<Value> {
    let (status, json) = self.execute(url, None, Option::None)?;
    if status.is_success() {
      Ok(json)
    } else {
      Err(HttpClientError.into())
    }
  }

  pub fn post(&self, url: &str, body: &str) -> Result<Value> {
    let (status, json) = self.execute(url, None, Some(body))?;
    if status.is_success() {
      Ok(json)
    } else {
      Err(HttpClientError.into())
    }
  }

  pub fn execute(
    &self,
    url: &str,
    headers: Option<Vec<Header>>,
    body: Option<&str>,
  ) -> Result<(Status, Value)> {
    debug!("Requesting: {}", url);

    trace!("Request headers: {:?}", &headers);
    trace!("Request body: {:?}", &body);

    if url.starts_with("http://") {
      warn!("Plain HTTP requested!");
      return Err(HttpClientError.into());
    }

    let mut request = if body.is_some() {
      self.agent.post(url)
    } else {
      self.agent.get(url)
    };

    if let Some(headers) = headers {
      for (name, value) in &headers {
        request = request.set(name, value);
      }
    }

    let result = if let Some(body) = body {
      request.send_string(body)
    } else {
      request.call()
    };

    let response = match result {
      Ok(response) => response,
      Err(ureq::Error::Status(_code, response)) => response,
      Err(err) => {
        debug!("Request failed!");
        return Err(err.into());
      }
    };

    let status: Status = response.status();

    trace!("Response: {}", status);

    if !status.is_success() {
      debug!("Request not successful: {}", status);
    }

    let json: Value = match from_reader::<_, Value>(response.into_reader()) {
      Ok(json) => {
        debug!("Response JSON: {}", to_string_pretty(&json)?);
        json
      }
      Err(err) => {
        debug!("Response JSON could not be parsed: {}", err);
        Value::Null
      }
    };

    return Ok((status, json));
  }
}

pub type Status = u16;

pub trait StatusExt {
  fn is_success(&self) -> bool;
}

impl StatusExt for Status {
  fn is_success(&self) -> bool {
    return *self >= 200 && *self < 400;
  }
}
