use regex::Regex;
use serde_derive::Deserialize;
use serde_derive::Serialize;

pub trait Named {
    fn name(&self) -> &String;
}

pub trait Identifiable {
    fn id(&self) -> &String;

    fn subscription_id(&self) -> Option<&str> {
        lazy_static! {
            static ref SUBSCRIPTION_RE: Regex = Regex::new(r"^/subscriptions/([^/]+)").unwrap();
        }
        match SUBSCRIPTION_RE.captures(self.id()) {
            Some(captures) => return Some(captures.get(1).unwrap().as_str()),
            None => return None,
        }
    }

    fn resource_group(&self) -> Option<&str> {
        lazy_static! {
            static ref RESOURCE_GROUP_RE: Regex = Regex::new(r"/resourceGroups/([^/]+)").unwrap();
        }
        match RESOURCE_GROUP_RE.captures(self.id()) {
            Some(captures) => return Some(captures.get(1).unwrap().as_str()),
            None => return None,
        }
    }
}

macro_rules! object {
    ($($name:ident),*) => (
        $(impl Named for $name {
            fn name(&self) -> &String { return &self.name; }
        }
        impl Identifiable for $name {
            fn id(&self) -> &String { return &self.id; }
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
    pub kind: Option<String>,
    pub location: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpAddress {
    pub id: String,
    pub name: String,
    pub properties: IpAddressProperties,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpAddressProperties {
    #[serde(rename = "ipAddress")]
    pub ip_address: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    pub entry: DnsRecordEntry,
}

#[derive(Debug, Clone, Serialize)]
pub enum DnsRecordEntry {
    A(Vec<String>),
    CNAME(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Costs {
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
            return &self.id;
        }
    }

    #[test]
    fn test_subscription_id() {
        assert_eq!(
            Some("123"),
            TestIdentifiable {
                id: "/subscriptions/123/test".to_owned()
            }
            .subscription_id()
        );
    }

    #[test]
    fn test_resource_group() {
        assert_eq!(
            Some("test"),
            TestIdentifiable {
                id: "/subscriptions/abc/resourceGroups/test".to_owned()
            }
            .resource_group()
        );
    }
}
