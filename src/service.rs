use std::net::IpAddr;
use std::str::from_utf8;

use base64::decode;
use serde_derive::Deserialize;
use serde_json::json;
use serde_json::Value;
use url::Url;
use yaml_rust::Yaml;
use yaml_rust::YamlLoader;

use crate::client::Client;
use crate::client::Request;
use crate::error::AppError::ServiceError;
use crate::http::Header;
use crate::http::Http;
use crate::object::AgentPool;
use crate::object::Costs;
use crate::object::DnsRecord;
use crate::object::DnsRecordEntry;
use crate::object::IpAddress;
use crate::object::KubernetesMetadata;
use crate::object::KubernetesObject;
use crate::object::ManagedCluster;
use crate::object::Resource;
use crate::object::ResourceGroup;
use crate::object::Subscription;
use crate::utils::Result;
use crate::utils::ValueExt;

pub const TYPE_DNS_ZONE: &'static str = "Microsoft.Network/dnsZones";

pub struct Service {
    client: Client,
    filter: Filter,
}

#[derive(Debug)]
pub enum Timeframe {
    MonthToDate,
    Custom { from: String, to: String },
}

pub struct Filter {
    filter: Option<String>,
}

impl Filter {
    pub fn new(filter: Option<&str>) -> Self {
        Filter {
            filter: filter.map(&str::to_lowercase),
        }
    }

    pub fn matches(&self, s: &Subscription) -> bool {
        match &self.filter {
            Some(filter) => {
                &s.subscription_id.to_lowercase() == filter
                    || s.name.to_lowercase().contains(filter)
            }
            None => true,
        }
    }
}

const DEFAULT_PREFIX: &'static str = "https://management.azure.com/";
const DEFAULT_RESOURCE: &'static str = "https://management.core.windows.net/";

impl Service {
    pub fn new(client: Client, filter: Filter) -> Service {
        return Service { client, filter };
    }

    pub fn get(&self, request: &str, resource: &str) -> Result<Value> {
        let url = &Service::to_url(request);
        if Service::is_azure(url)? {
            self.with_request(url, resource, |request| request.get_raw())
        } else {
            self.client.http().get(url)?.success()
        }
    }

    pub fn post(&self, request: &str, resource: &str, body: &str) -> Result<Value> {
        let url = &Service::to_url(request);
        if Service::is_azure(url)? {
            self.with_request(url, resource, |request| request.body(body).post_raw())
        } else {
            self.client.http().post(url, body)?.success()
        }
    }

    fn to_url(request: &str) -> String {
        if request.starts_with("https://") {
            request.to_owned()
        } else {
            format!("{}{}", DEFAULT_PREFIX, request)
        }
    }

    fn is_azure(url: &str) -> Result<bool> {
        Url::parse(url).map_err(|err| err.into()).map(|url| {
            url.host_str()
                .map(|host| host == "azure.com" || host.ends_with(".azure.com"))
                .unwrap_or(false)
        })
    }

    fn with_request(
        &self,
        url: &str,
        resource: &str,
        function: impl Fn(Request) -> Result<Value>,
    ) -> Result<Value> {
        let resource = if resource.is_empty() {
            DEFAULT_RESOURCE
        } else {
            resource
        };
        function(self.client.new_request(url, resource))
    }

    pub fn get_subscriptions(&self) -> Result<Vec<Subscription>> {
        let url = "https://management.azure.com/subscriptions?api-version=2016-06-01";
        let mut subscriptions: Vec<Subscription> = self
            .client
            .new_request(url, DEFAULT_RESOURCE)
            .get_list()?
            .into_iter()
            .filter(|subscription| self.filter.matches(&subscription))
            .collect();
        subscriptions.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(subscriptions)
    }

    pub fn get_resource_groups(&self, subscription_id: &str) -> Result<Vec<ResourceGroup>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourcegroups?api-version=2018-05-01",
            subscription_id
        );
        self.client
            .new_request(&url, DEFAULT_RESOURCE)
            .get_list()
            .map(|mut list: Vec<ResourceGroup>| {
                list.sort_by(|a, b| a.name.cmp(&b.name));
                list
            })
    }

    pub fn get_resources(&self, subscription_id: &str) -> Result<Vec<Resource>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resources?api-version=2018-05-01",
            subscription_id
        );
        self.client.new_request(&url, DEFAULT_RESOURCE).get_list()
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
        self.client
            .new_request(&url, DEFAULT_RESOURCE)
            .query("$filter", &format!("resourceType eq '{}'", resource_type))
            .get_list()
    }

    pub fn get_clusters(&self, subscription_id: &str) -> Result<Vec<ManagedCluster>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.ContainerService/managedClusters?api-version=2021-03-01",
            subscription_id
        );
        self.client.new_request(&url, DEFAULT_RESOURCE).get_list()
    }

    pub fn get_agent_pools(&self, cluster_id: &str) -> Result<Vec<AgentPool>> {
        let url = format!(
            "https://management.azure.com{}/agentPools?api-version=2021-03-01",
            cluster_id
        );
        self.client.new_request(&url, DEFAULT_RESOURCE).get_list()
    }

    pub fn get_cluster_kubeconfig(&self, cluster_id: &str) -> Result<String> {
        #[derive(Debug, Clone, Deserialize)]
        pub struct ClusterCredentials {
            pub kubeconfigs: Vec<ClusterCredentialsEntry>,
        }

        #[derive(Debug, Clone, Deserialize)]
        pub struct ClusterCredentialsEntry {
            pub name: String,
            pub value: String,
        }

        let credentials: ClusterCredentials = {
            let url = format!(
                "https://management.azure.com{}/listClusterUserCredential?api-version=2021-03-01",
                cluster_id
            );
            self.client.new_request(&url, DEFAULT_RESOURCE).post()?
        };

        let entry = credentials
            .kubeconfigs
            .iter()
            .find(|e| e.name == "clusterUser")
            .ok_or(ServiceError("entry 'clusterUser' not found"))?;

        let kubeconfig = from_utf8(&decode(&entry.value)?)?.to_owned();
        debug!("kubeconfig: {}", kubeconfig);

        Ok(kubeconfig)
    }

    pub fn get_kubernetes_objects(
        &self,
        kubeconfig: &str,
        all_resources: bool,
    ) -> Result<Vec<KubernetesObject>> {
        let cluster = KubernetesCluster::parse(kubeconfig)?;

        let http = Http::for_certificate_authority(&cluster.certificate_authority)?
            .with_url(cluster.server.clone());

        let http = match &cluster.auth {
            KubernetesAuthentication::BearerToken(token) => {
                http.with_headers(vec![Header::auth_bearer(&token), Header::content_json()])
            }
            KubernetesAuthentication::AccessToken {
                client_id,
                resource,
            } => {
                let token_set = self.client.get_token_set(&client_id, &resource)?;
                http.with_headers(vec![
                    Header::auth_bearer(token_set.access_token.token()),
                    Header::content_json(),
                ])
            }
        };

        let mut objects = vec![];
        Self::get_kubernetes_services(&http, &mut objects)?;
        Self::get_kubernetes_deployments(&http, &mut objects)?;

        if !all_resources {
            objects.retain(|object| {
                let metadata = object.metadata();
                metadata.namespace != "kube-system"
                    && metadata
                        .labels
                        .get("provider")
                        .filter(|p| p.as_str() == "kubernetes")
                        .is_none()
            });
        }

        Ok(objects)
    }

    fn get_kubernetes_services(http: &Http, objects: &mut Vec<KubernetesObject>) -> Result<()> {
        let json = http
            .execute("/api/v1/services?limit=200", None, None)?
            .success()?;

        fn to_service(json: &Value) -> Result<KubernetesObject> {
            let metadata = json["metadata"].clone().to::<KubernetesMetadata>()?;
            let service_type = json["spec"]["type"].string()?;
            let mut ip_addresses = vec![];
            if let Some(ip) = json["spec"]["clusterIP"].as_str() {
                ip_addresses.push(ip.to_owned());
            }
            if let Some(ip_arr) = json["spec"]["externalIPs"].as_array() {
                for ip in ip_arr {
                    ip_addresses.push(ip.string()?);
                }
            }
            if let Some(ingress_arr) = json["status"]["loadBalancer"]["ingress"].as_array() {
                for ingress in ingress_arr {
                    if let Some(ip) = ingress["ip"].as_str() {
                        ip_addresses.push(ip.to_owned());
                    }
                }
            }
            ip_addresses.retain(|ip| ip != "" && ip != "None");
            let ip_addresses = ip_addresses
                .into_iter()
                .map(|ip| Ok(ip.parse::<IpAddr>()?))
                .collect::<Result<Vec<IpAddr>>>()?;
            Ok(KubernetesObject::Service {
                metadata,
                service_type,
                ip_addresses,
            })
        }

        for item in json["items"].to_array()? {
            objects.push(match to_service(item) {
                Ok(service) => service,
                Err(err) => {
                    debug!("Failed to parse JSON: {}", item.to_string());
                    return Err(err);
                }
            });
        }

        Ok(())
    }

    fn get_kubernetes_deployments(http: &Http, objects: &mut Vec<KubernetesObject>) -> Result<()> {
        let json = http
            .execute("/apis/apps/v1/deployments?limit=200", None, None)?
            .success()?;

        fn to_deployment(json: &Value) -> Result<KubernetesObject> {
            let metadata = json["metadata"].clone().to::<KubernetesMetadata>()?;
            let target = json["status"]["replicas"].as_u64().unwrap_or(0);
            let ready = json["status"]["readyReplicas"].as_u64().unwrap_or(0);
            Ok(KubernetesObject::Deployment {
                metadata,
                target,
                ready,
            })
        }

        for item in json["items"].to_array()? {
            objects.push(match to_deployment(item) {
                Ok(deployment) => deployment,
                Err(err) => {
                    debug!("Failed to parse JSON: {}", item.to_string());
                    return Err(err);
                }
            });
        }

        Ok(())
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
            .ok_or(ServiceError("response is not an array"))?
            .iter()
            .filter_map(|row| {
                if let (Some(id), Some(name), Some(ip_address)) = (
                    row["id"].as_str(),
                    row["name"].as_str(),
                    row["properties"]["ipAddress"].as_str(),
                ) {
                    return Some(IpAddress {
                        id: id.to_owned(),
                        name: name.to_owned(),
                        ip_address: ip_address.to_owned(),
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
            "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Network/dnsZones/{}/recordsets?api-version=2018-05-01",
            subscription_id,
            resource_group,
            zone,
        );

        let json = self.client.new_request(&url, DEFAULT_RESOURCE).get_raw()?;

        let records = json
            .as_array()
            .ok_or(ServiceError("response is not an array"))?
            .iter()
            .filter_map(|row| {
                let (id, name) =
                    if let (Some(id), Some(name)) = (row["id"].as_str(), row["name"].as_str()) {
                        (id.to_owned(), name.to_owned())
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
                        .map(str::to_owned)
                        .collect();
                    DnsRecordEntry::A(ip_addresses)
                } else if let Some(cname) = row["properties"]["CNAMERecord"]["cname"].as_str() {
                    DnsRecordEntry::CNAME(cname.to_owned())
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
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.CostManagement/query?api-version=2024-08-01",
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
            .post_raw()?;

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
            return Err(ServiceError("column not found").into());
        }

        let resource_group_col = find_column(&json, "ResourceGroup")?;
        let costs_col = find_column(&json, "PreTaxCost")?;
        let currency_col = find_column(&json, "Currency")?;

        let items = json["properties"]["rows"]
            .as_array()
            .ok_or(ServiceError("response is not an array"))?
            .iter()
            .filter_map(|value| {
                if let Some(arr) = value.as_array() {
                    if let (Some(resource_group), Some(costs), Some(currency)) = (
                        arr.get(resource_group_col).and_then(Value::as_str),
                        arr.get(costs_col).and_then(Value::as_f64),
                        arr.get(currency_col).and_then(Value::as_str),
                    ) {
                        return Some(Costs {
                            resource_group: resource_group.to_owned(),
                            costs,
                            currency: currency.to_owned(),
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

pub struct KubernetesCluster {
    pub server: String,
    pub certificate_authority: String,
    pub auth: KubernetesAuthentication,
}

#[derive(Debug, PartialEq)]
pub enum KubernetesAuthentication {
    BearerToken(String),
    AccessToken { client_id: String, resource: String },
}

impl KubernetesCluster {
    pub fn parse(kubeconfig: &str) -> Result<KubernetesCluster> {
        let err = || ServiceError("invalid kubeconfig structure");

        let configs = YamlLoader::load_from_str(kubeconfig)?;
        let config = configs.get(0).ok_or_else(err)?;

        fn get_entry<'a>(obj: &'a Yaml, name: &Yaml) -> Result<&'a Yaml> {
            let err = || ServiceError("cannot find kubeconfig entry");
            let name = name.as_str().ok_or_else(err)?;
            Ok(obj
                .as_vec()
                .ok_or_else(err)?
                .iter()
                .find(|c| c["name"].as_str() == Some(name))
                .ok_or_else(err)?)
        }

        let current_context = &config["current-context"];
        let context = &get_entry(&config["contexts"], &current_context)?["context"];

        let cluster = &get_entry(&config["clusters"], &context["cluster"])?["cluster"];
        let user = &get_entry(&config["users"], &context["user"])?["user"];

        let to_str = |yaml: &Yaml| yaml.as_str().ok_or_else(err).map(|s| s.to_owned());

        let ca = from_utf8(&decode(&to_str(&cluster["certificate-authority-data"])?)?)?.to_owned();

        let auth = if !user["auth-provider"].is_badvalue() {
            KubernetesAuthentication::AccessToken {
                client_id: to_str(&user["auth-provider"]["config"]["client-id"])?,
                resource: to_str(&user["auth-provider"]["config"]["apiserver-id"])?,
            }
        } else {
            KubernetesAuthentication::BearerToken(to_str(&user["token"])?)
        };

        Ok(KubernetesCluster {
            server: to_str(&cluster["server"])?,
            certificate_authority: ca,
            auth,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::KubernetesAuthentication;
    use super::KubernetesCluster;

    #[test]
    fn test_parse_kubeconfig() {
        let data = r#"current-context: context0
contexts:
- name: context0
  context:
    cluster: cluster0
    user: user0
clusters:
- name: cluster0
  cluster:
    certificate-authority-data: Q0E=
    server: http://localhost
users:
- name: user0
  user:
    auth-provider:
      config:
        client-id: abc-def
        apiserver-id: 123-456
"#;
        let parsed = KubernetesCluster::parse(data);
        assert_eq!(true, parsed.is_ok());
        let cluster = parsed.unwrap();
        assert_eq!("http://localhost", cluster.server);
        assert_eq!("CA", cluster.certificate_authority);
        assert_eq!(
            KubernetesAuthentication::AccessToken {
                client_id: "abc-def".to_owned(),
                resource: "123-456".to_owned()
            },
            cluster.auth
        );
    }
}
