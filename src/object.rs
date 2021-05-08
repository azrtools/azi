use regex::Regex;
use serde_derive::Deserialize;
use serde_derive::Serialize;

use crate::error::AppError::ParseError;
use crate::utils::Result;

pub trait Named {
    fn name(&self) -> &String;
}

pub trait Identifiable {
    fn id(&self) -> &String;

    fn subscription_id(&self) -> Result<&str> {
        lazy_static! {
            static ref SUBSCRIPTION_RE: Regex = Regex::new(r"^/subscriptions/([^/]+)").unwrap();
        }
        match SUBSCRIPTION_RE.captures(self.id()) {
            Some(captures) => Ok(captures.get(1).unwrap().as_str()),
            None => Err(ParseError("invalid id!".to_owned()).into()),
        }
    }

    fn resource_group(&self) -> Result<&str> {
        lazy_static! {
            static ref RESOURCE_GROUP_RE: Regex = Regex::new(r"/resourceGroups/([^/]+)").unwrap();
        }
        match RESOURCE_GROUP_RE.captures(self.id()) {
            Some(captures) => Ok(captures.get(1).unwrap().as_str()),
            None => Err(ParseError("invalid id!".to_owned()).into()),
        }
    }
}

macro_rules! object {
    ($($name:ident),*) => (
        $(impl Named for $name {
            fn name(&self) -> &String { &self.name }
        }
        impl Identifiable for $name {
            fn id(&self) -> &String { &self.id }
        })*
    )
}

object!(Subscription, ResourceGroup, Resource, IpAddress, DnsRecord);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    #[serde(rename = "subscriptionId")]
    pub subscription_id: String,
    #[serde(rename = "displayName")]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceGroup {
    pub id: String,
    pub location: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub location: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManagedCluster {
    pub id: String,
    pub location: String,
    pub name: String,
    pub properties: ManagedClusterProperties,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManagedClusterProperties {
    #[serde(rename = "kubernetesVersion")]
    pub kubernetes_version: String,
    #[serde(rename = "agentPoolProfiles")]
    pub agent_pool_profiles: Vec<AgentPoolProfile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentPoolProfile {
    pub name: String,
    #[serde(rename = "minCount")]
    pub min_count: Option<u64>,
    #[serde(rename = "maxCount")]
    pub max_count: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClusterCredentials {
    pub kubeconfigs: Vec<ClusterCredentialsEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClusterCredentialsEntry {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentPool {
    pub name: String,
    pub properties: AgentPoolProperties,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentPoolProperties {
    pub count: u64,
    #[serde(rename = "vmSize")]
    pub vm_size: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IpAddress {
    pub id: String,
    pub name: String,
    #[serde(rename = "ipAddress")]
    pub ip_address: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    pub fqdn: String,
    pub entry: DnsRecordEntry,
}

#[derive(Debug, Clone, Serialize)]
pub enum DnsRecordEntry {
    A(Vec<String>),
    CNAME(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Costs {
    #[serde(rename = "resourceGroup")]
    pub resource_group: String,
    pub costs: f64,
    pub currency: String,
}

#[cfg(test)]
mod tests {
    use super::Identifiable;

    struct TestIdentifiable {
        id: String,
    }

    impl Identifiable for TestIdentifiable {
        fn id(&self) -> &String {
            &self.id
        }
    }

    #[test]
    fn test_subscription_id() {
        assert_eq!(
            "123",
            TestIdentifiable {
                id: "/subscriptions/123/test".to_owned()
            }
            .subscription_id()
            .unwrap()
        );
    }

    #[test]
    fn test_resource_group() {
        assert_eq!(
            "test",
            TestIdentifiable {
                id: "/subscriptions/abc/resourceGroups/test".to_owned()
            }
            .resource_group()
            .unwrap()
        );
    }
}
