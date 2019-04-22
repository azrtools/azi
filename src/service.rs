use std::error::Error;

use serde_json::Value;

use crate::client::Client;
use crate::error::AppError::ServiceError;
use crate::object::Resource;
use crate::object::ResourceGroup;
use crate::object::Subscription;

type Result<T> = std::result::Result<T, Box<Error>>;

pub struct Service {
    client: Client,
}

const DEFAULT_PREFIX: &'static str = "https://management.azure.com/";
const DEFAULT_RESOURCE: &'static str = "https://management.core.windows.net/";

impl Service {
    pub fn new(client: Client) -> Service {
        return Service { client };
    }

    pub fn get(&self, request: &str, resource: &str) -> Result<Value> {
        let resource = if resource.is_empty() {
            DEFAULT_RESOURCE
        } else {
            resource
        };
        if request.starts_with("http://") {
            warn!("Plain HTTP requested!");
            return Err(ServiceError.into());
        } else if request.starts_with("https://") {
            return self.client.get_raw(request, resource);
        } else {
            let request = format!("{}/{}", DEFAULT_PREFIX, request);
            return self.client.get_raw(&request, resource);
        }
    }

    pub fn get_subscriptions(&self) -> Result<Vec<Subscription>> {
        let url = "https://management.azure.com/subscriptions?api-version=2016-06-01";
        return self.client.get_list(url, DEFAULT_RESOURCE);
    }

    pub fn get_resource_groups(&self, subscription_id: &str) -> Result<Vec<ResourceGroup>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourcegroups?api-version=2018-05-01",
            subscription_id
        );
        return self.client.get_list(&url, DEFAULT_RESOURCE);
    }

    pub fn get_resources(&self, subscription_id: &str) -> Result<Vec<Resource>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resources?api-version=2018-05-01",
            subscription_id
        );
        return self.client.get_list(&url, DEFAULT_RESOURCE);
    }
}
