use std::error::Error;

use serde::de::DeserializeOwned;
use serde_json::from_value;

use crate::client::Client;
use crate::error::AppError::ServiceError;
use crate::object::ResourceGroup;
use crate::object::Subscription;

type Result<T> = std::result::Result<T, Box<Error>>;

pub struct Service {
    client: Client,
}

const DEFAULT_RESOURCE: &'static str = "https://management.core.windows.net/";

impl Service {
    pub fn new(client: Client) -> Service {
        return Service { client };
    }

    pub fn get_subscriptions(&self) -> Result<Vec<Subscription>> {
        let url = "https://management.azure.com/subscriptions?api-version=2016-06-01";
        return self.get_list(url, DEFAULT_RESOURCE);
    }

    pub fn get_resource_groups(&self, subscription_id: &str) -> Result<Vec<ResourceGroup>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourcegroups?api-version=2018-05-01",
            subscription_id
        );
        return self.get_list(&url, DEFAULT_RESOURCE);
    }

    fn get_list<T>(&self, url: &str, resource: &str) -> Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let json = self.client.get(url, resource)?;
        if let Some(arr) = json["value"].as_array() {
            let mut vec = Vec::new();
            for entry in arr {
                let item: T = from_value(entry.clone())?;
                vec.push(item);
            }
            return Ok(vec);
        }
        return Err(ServiceError.into());
    }
}
