use std::error::Error;

use serde_json::Value;

use crate::client::Client;
use crate::error::AppError::ServiceError;
use crate::object::DnsRecord;
use crate::object::IpAddress;
use crate::object::Resource;
use crate::object::ResourceGroup;
use crate::object::Subscription;

pub const TYPE_DNS_ZONE: &'static str = "Microsoft.Network/dnsZones";

pub struct Service {
    client: Client,
}

type Result<T> = std::result::Result<T, Box<Error>>;

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
            return self.client.new_request(request, resource).get_raw();
        } else {
            let request = format!("{}/{}", DEFAULT_PREFIX, request);
            return self.client.new_request(&request, resource).get_raw();
        }
    }

    pub fn get_subscriptions(&self) -> Result<Vec<Subscription>> {
        let url = "https://management.azure.com/subscriptions?api-version=2016-06-01";
        return self.client.new_request(url, DEFAULT_RESOURCE).get_list();
    }

    pub fn get_resource_groups(&self, subscription_id: &str) -> Result<Vec<ResourceGroup>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourcegroups?api-version=2018-05-01",
            subscription_id
        );
        return self.client.new_request(&url, DEFAULT_RESOURCE).get_list();
    }

    pub fn get_resources(&self, subscription_id: &str) -> Result<Vec<Resource>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resources?api-version=2018-05-01",
            subscription_id
        );
        return self.client.new_request(&url, DEFAULT_RESOURCE).get_list();
    }

    pub fn get_resources_by_type(
        &self,
        subscription_id: &str,
        resource_type: &str,
    ) -> Result<Vec<Resource>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resources?api-version=2018-05-01",
            subscription_id
        );
        return self
            .client
            .new_request(&url, DEFAULT_RESOURCE)
            .query("$filter", &format!("resourceType eq '{}'", resource_type))
            .get_list();
    }

    pub fn get_ip_addresses(
        &self,
        subscription_id: &str,
        resource_group: &str,
    ) -> Result<Vec<IpAddress>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Network/publicIPAddresses?api-version=2018-11-01",
            subscription_id,
            resource_group
        );
        return self.client.new_request(&url, DEFAULT_RESOURCE).get_list();
    }

    pub fn get_dns_records(
        &self,
        subscription_id: &str,
        resource_group: &str,
        zone: &str,
    ) -> Result<Vec<DnsRecord>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Network/dnsZones/{}/recordsets?api-version=2018-03-01-preview",
            subscription_id,
            resource_group,
            zone,
        );
        return self.client.new_request(&url, DEFAULT_RESOURCE).get_list();
    }
}
