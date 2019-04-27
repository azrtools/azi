use std::error::Error;

use serde_json::json;
use serde_json::Value;

use crate::client::Client;
use crate::client::Request;
use crate::error::AppError::ServiceError;
use crate::object::Costs;
use crate::object::DnsRecord;
use crate::object::DnsRecordEntry;
use crate::object::IpAddress;
use crate::object::Resource;
use crate::object::ResourceGroup;
use crate::object::Subscription;

pub const TYPE_DNS_ZONE: &'static str = "Microsoft.Network/dnsZones";

pub struct Service {
    client: Client,
}

pub enum Timeframe {
    MonthToDate,
    Custom { from: String, to: String },
}

type Result<T> = std::result::Result<T, Box<Error>>;

const DEFAULT_PREFIX: &'static str = "https://management.azure.com/";
const DEFAULT_RESOURCE: &'static str = "https://management.core.windows.net/";

impl Service {
    pub fn new(client: Client) -> Service {
        return Service { client };
    }

    pub fn get(&self, request: &str, resource: &str) -> Result<Value> {
        return self.with_request(request, resource, |request| request.get_raw());
    }

    pub fn post(&self, request: &str, resource: &str, body: &str) -> Result<Value> {
        return self.with_request(request, resource, |request| request.body(body).post());
    }

    fn with_request(
        &self,
        request: &str,
        resource: &str,
        function: impl Fn(Request) -> Result<Value>,
    ) -> Result<Value> {
        if request.starts_with("http://") {
            warn!("Plain HTTP requested!");
            return Err(ServiceError.into());
        }

        let resource = if resource.is_empty() {
            DEFAULT_RESOURCE
        } else {
            resource
        };

        if request.starts_with("https://") {
            return function(self.client.new_request(request, resource));
        } else {
            let request = format!("{}/{}", DEFAULT_PREFIX, request);
            return function(self.client.new_request(&request, resource));
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

    pub fn get_ip_addresses(&self, subscription_id: &str) -> Result<Vec<IpAddress>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Network/publicIPAddresses?api-version=2018-11-01",
            subscription_id
        );
        return Ok(self
            .client
            .new_request(&url, DEFAULT_RESOURCE)
            .get_raw()?
            .as_array()
            .ok_or(ServiceError)?
            .iter()
            .filter_map(|row| {
                if let (Some(id), Some(name), Some(ip_address)) = (
                    row["id"].as_str(),
                    row["name"].as_str(),
                    row["properties"]["ipAddress"].as_str(),
                ) {
                    return Some(IpAddress {
                        id: id.to_string(),
                        name: name.to_string(),
                        ip_address: ip_address.to_string(),
                    });
                } else {
                    trace!("Invalid row, missing id or name: {:?}", row);
                    return None;
                }
            })
            .collect());
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

        let json = self.client.new_request(&url, DEFAULT_RESOURCE).get_raw()?;

        let records = json
            .as_array()
            .ok_or(ServiceError)?
            .iter()
            .filter_map(|row| {
                let (id, name) =
                    if let (Some(id), Some(name)) = (row["id"].as_str(), row["name"].as_str()) {
                        (id.to_string(), name.to_string())
                    } else {
                        trace!("Invalid row, missing id or name: {:?}", row);
                        return None;
                    };
                let fqdn = if name == "@" {
                    zone.to_owned()
                } else {
                    format!("{}.{}", name, zone)
                };
                let entry = if let Some(a_records) = row["properties"]["ARecords"].as_array() {
                    let ip_addresses: Vec<String> = a_records
                        .iter()
                        .filter_map(|row| row["ipv4Address"].as_str())
                        .map(str::to_string)
                        .collect();
                    DnsRecordEntry::A(ip_addresses)
                } else if let Some(cname) = row["properties"]["CNAMERecord"]["cname"].as_str() {
                    DnsRecordEntry::CNAME(cname.to_string())
                } else {
                    trace!("Invalid row, unknown record type: {:?}", row);
                    return None;
                };
                return Some(DnsRecord {
                    id,
                    name,
                    fqdn,
                    entry,
                });
            })
            .collect();

        return Ok(records);
    }

    pub fn get_costs(&self, subscription_id: &str, timeframe: &Timeframe) -> Result<Vec<Costs>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.CostManagement/query?api-version=2019-01-01",
            subscription_id
        );

        let body = json!({
            "type": "Usage",
            "timeframe": match timeframe {
                Timeframe::MonthToDate => "MonthToDate",
                Timeframe::Custom { .. } => "Custom"
            },
            "timePeriod": match timeframe {
                Timeframe::Custom { from, to } => json!({ "from": from, "to": to }),
                _ => Value::Null
            },
            "dataset": {
            "granularity": "Monthly",
            "aggregation": {
              "totalCost": {
                "name": "PreTaxCost",
                "function": "Sum"
              }
            },
            "grouping": [
              {
                "type": "Dimension",
                "name": "ResourceGroup"
              }
            ]
          }
        });

        let json = self
            .client
            .new_request(&url, DEFAULT_RESOURCE)
            .body(&body.to_string())
            .post()?;

        fn find_column(json: &Value, name: &str) -> Result<usize> {
            if let Some(columns) = json["properties"]["columns"].as_array() {
                for (i, column) in columns.iter().enumerate() {
                    match column["name"].as_str() {
                        Some(n) if n == name => return Ok(i),
                        _ => (),
                    }
                }
            }
            warn!("Column not found: {}", name);
            return Err(ServiceError.into());
        }

        let resource_group_col = find_column(&json, "ResourceGroup")?;
        let costs_col = find_column(&json, "PreTaxCost")?;
        let currency_col = find_column(&json, "Currency")?;

        let items = json["properties"]["rows"]
            .as_array()
            .ok_or(ServiceError)?
            .iter()
            .filter_map(|value| {
                if let Some(arr) = value.as_array() {
                    if let (Some(resource_group), Some(costs), Some(currency)) = (
                        arr.get(resource_group_col).and_then(Value::as_str),
                        arr.get(costs_col).and_then(Value::as_f64),
                        arr.get(currency_col).and_then(Value::as_str),
                    ) {
                        return Some(Costs {
                            resource_group: resource_group.to_string(),
                            costs,
                            currency: currency.to_string(),
                        });
                    }
                }
                warn!("Invalid value: {:?}", value);
                return None;
            })
            .collect::<Vec<_>>();

        return Ok(items);
    }
}
