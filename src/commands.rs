use std::collections::HashMap;

use serde_derive::Serialize;
use serde_json::Value;

use crate::object::Costs;
use crate::object::DnsRecord;
use crate::object::DnsRecordEntry;
use crate::object::Identifiable;
use crate::object::IpAddress;
use crate::object::KubernetesObject;
use crate::object::Resource;
use crate::object::ResourceGroup;
use crate::object::Subscription;
use crate::service::Service;
use crate::service::Timeframe;
use crate::service::TYPE_DNS_ZONE;
use crate::utils::Result;

pub struct Context<'c> {
    pub service: &'c Service,
}

#[derive(Serialize)]
pub struct ListResult {
    pub subscription: Subscription,
    #[serde(rename = "resourceGroups")]
    pub resource_groups: Vec<ResourceGroup>,
    pub resources: Vec<Resource>,
}

pub fn list(
    context: &Context,
    list_resources: bool,
    filter: Option<&String>,
) -> Result<Vec<ListResult>> {
    let service = &context.service;

    let mut results = vec![];

    for subscription in service.get_subscriptions()? {
        let mut resource_groups = service.get_resource_groups(&subscription.subscription_id)?;
        if !list_resources {
            if let Some(filter) = filter {
                resource_groups.retain(|group| group.name.contains(filter));
            }
        }

        let resources = if list_resources {
            let mut resources = service.get_resources(&subscription.subscription_id)?;
            if let Some(filter) = filter {
                resources.retain(|resource| resource.name.contains(filter));

                resource_groups.retain(|group| {
                    for resource in &resources {
                        if let Ok(resource_group) = resource.resource_group() {
                            if resource_group == group.name {
                                return true;
                            }
                        }
                    }
                    false
                });
            }
            resources
        } else {
            vec![]
        };

        if (list_resources && !resources.is_empty())
            || (!list_resources && !resource_groups.is_empty())
        {
            results.push(ListResult {
                subscription,
                resource_groups,
                resources,
            });
        }
    }

    return Ok(results);
}

#[derive(Serialize)]
pub struct ClusterResult {
    pub subscription: Subscription,
    pub clusters: Vec<Cluster>,
}

#[derive(Serialize)]
pub struct Cluster {
    pub id: String,
    pub name: String,
    pub version: String,
    pub agent_pools: Option<Vec<AgentPool>>,
    pub objects: Option<Vec<KubernetesObject>>,
}

#[derive(Serialize)]
pub struct AgentPool {
    pub name: String,
    pub count: u64,
    pub min_count: Option<u64>,
    pub max_count: Option<u64>,
    pub vm_size: String,
}

pub fn clusters(
    context: &Context,
    pools: bool,
    resources: bool,
    all_resources: bool,
    containers: bool,
    filter: Option<&String>,
) -> Result<Vec<ClusterResult>> {
    let service = &context.service;

    let mut results = vec![];

    for subscription in service.get_subscriptions()? {
        let mut managed_clusters = service.get_clusters(&subscription.subscription_id)?;
        if let Some(filter) = filter {
            managed_clusters.retain(|cluster| cluster.name.contains(filter));
        }

        if !managed_clusters.is_empty() {
            let clusters: Result<Vec<_>> = managed_clusters
                .into_iter()
                .map(|cluster| {
                    let agent_pools = if pools {
                        let agent_pools = service
                            .get_agent_pools(&cluster.id)?
                            .into_iter()
                            .map(|agent_pool| {
                                let profile = cluster
                                    .properties
                                    .agent_pool_profiles
                                    .iter()
                                    .find(|pool| pool.name == agent_pool.name);

                                AgentPool {
                                    name: agent_pool.name,
                                    count: agent_pool.properties.count,
                                    min_count: profile.and_then(|p| p.min_count),
                                    max_count: profile.and_then(|p| p.max_count),
                                    vm_size: agent_pool.properties.vm_size,
                                }
                            })
                            .collect();
                        Some(agent_pools)
                    } else {
                        None
                    };

                    let objects = if resources {
                        let kubeconfig = service.get_cluster_kubeconfig(
                            &cluster.id,
                            cluster.properties.fqdn.is_some()
                                && cluster.properties.private_fqdn.is_some(),
                        )?;
                        let objects =
                            service.get_kubernetes_objects(&kubeconfig, all_resources, containers);
                        match objects {
                            Ok(objects) => Some(objects),
                            Err(err) => {
                                warn!(
                                    "Failed to get Kubernetes resources for {}: {}",
                                    &cluster.name, err
                                );
                                None
                            }
                        }
                    } else {
                        None
                    };

                    Ok(Cluster {
                        id: cluster.id,
                        name: cluster.name,
                        version: cluster.properties.kubernetes_version,
                        agent_pools,
                        objects,
                    })
                })
                .collect();

            results.push(ClusterResult {
                subscription,
                clusters: clusters?,
            });
        }
    }

    return Ok(results);
}

#[derive(Serialize)]
pub struct Domain {
    pub name: String,
    pub entries: Vec<Option<DnsRecordEntry>>,
    #[serde(rename = "ipAddresses")]
    pub ip_addresses: Vec<DomainIpAddress>,
}

#[derive(Serialize)]
pub struct DomainIpAddress {
    #[serde(rename = "ipAddress")]
    pub ip_address: String,
    #[serde(rename = "resourceGroup")]
    pub resource_group: Option<ResourceGroup>,
}

pub fn domains(context: &Context, filter: Option<&String>) -> Result<Vec<Domain>> {
    let service = &context.service;

    let subscriptions = service.get_subscriptions()?;

    let mut records: Vec<DnsRecord> = vec![];
    for subscription in &subscriptions {
        for zone in service.get_resources_by_type(&subscription.subscription_id, TYPE_DNS_ZONE)? {
            records.extend(service.get_dns_records(
                &subscription.subscription_id,
                zone.resource_group()?,
                &zone.name,
            )?);
        }
    }

    let mut ip_to_group: HashMap<String, ResourceGroup> = HashMap::new();
    for subscription in &subscriptions {
        let groups = service.get_resource_groups(&subscription.subscription_id)?;
        let ips = service.get_ip_addresses(&subscription.subscription_id)?;
        for ip in ips {
            let group_name = ip.resource_group()?.to_lowercase();
            let group = groups
                .iter()
                .find(|group| group.name.to_lowercase() == group_name);
            if let Some(group) = group {
                ip_to_group.insert(ip.ip_address, group.clone());
            }
        }
    }

    fn equals(fqdn1: &str, fqdn2: &str) -> bool {
        fqdn1 == fqdn2
            || (fqdn1.ends_with(".") && &fqdn1[..fqdn1.len() - 1] == fqdn2)
            || (fqdn2.ends_with(".") && fqdn1 == &fqdn2[..fqdn2.len() - 1])
    }

    let mut domain_names: Vec<&String> = (&records).iter().map(|record| &record.fqdn).collect();

    if let Some(filter) = filter {
        domain_names.retain(|domain| domain.contains(filter));
    } else {
        for record in &records {
            match &record.entry {
                DnsRecordEntry::CNAME(cname) => {
                    domain_names.retain(|&domain| !equals(domain, cname));
                }
                _ => (),
            }
        }
    }

    domain_names.sort();

    const MAX_DEPTH: usize = 5;

    fn resolve_entries<'e>(
        entries: &'e mut Vec<Option<DnsRecordEntry>>,
        records: &'e Vec<DnsRecord>,
        domain_name: &str,
        depth: usize,
    ) {
        for record in records {
            if equals(&record.fqdn, domain_name) {
                match &record.entry {
                    DnsRecordEntry::CNAME(cname) => {
                        if depth >= MAX_DEPTH {
                            entries.push(None);
                        } else {
                            entries.push(Some(record.entry.clone()));
                            resolve_entries(entries, records, cname, depth + 1);
                        }
                    }
                    DnsRecordEntry::A(_) => {
                        entries.push(Some(record.entry.clone()));
                    }
                }
            }
        }
    }

    let mut domains = vec![];

    for domain_name in &domain_names {
        let mut entries = vec![];
        resolve_entries(&mut entries, &records, domain_name, 0);

        let mut ip_addresses = vec![];
        if let Some(Some(entry)) = entries.last() {
            match entry {
                DnsRecordEntry::A(ip_addrs) => {
                    for ip in ip_addrs {
                        ip_addresses.push(DomainIpAddress {
                            ip_address: ip.clone(),
                            resource_group: ip_to_group.get(ip).map(|r| r.clone()),
                        });
                    }
                }
                _ => (),
            }
        }

        domains.push(Domain {
            name: domain_name.to_string(),
            entries,
            ip_addresses,
        });
    }

    return Ok(domains);
}

#[derive(Serialize)]
pub struct DnsResult {
    pub zone: Resource,
    pub records: Vec<DnsRecord>,
}

pub fn dns(context: &Context) -> Result<Vec<DnsResult>> {
    let service = &context.service;

    let subscriptions = service.get_subscriptions()?;

    let mut zones = vec![];
    for subscription in &subscriptions {
        zones.extend(service.get_resources_by_type(&subscription.subscription_id, TYPE_DNS_ZONE)?);
    }

    let mut results = vec![];

    for zone in &zones {
        let records =
            service.get_dns_records(zone.subscription_id()?, zone.resource_group()?, &zone.name)?;
        results.push(DnsResult {
            zone: zone.clone(),
            records,
        });
    }

    return Ok(results);
}

#[derive(Serialize)]
pub struct IpResult {
    pub subscription: Subscription,
    #[serde(rename = "resourceGroups")]
    pub resource_groups: Vec<IpResultResourceGroup>,
}

#[derive(Serialize)]
pub struct IpResultResourceGroup {
    #[serde(rename = "resourceGroup")]
    pub resource_group: ResourceGroup,
    #[serde(rename = "ipAddresses")]
    pub ip_addresses: Vec<IpAddress>,
}

pub fn ip(context: &Context) -> Result<Vec<IpResult>> {
    let mut result = vec![];

    let service = &context.service;
    let subscriptions = service.get_subscriptions()?;
    for subscription in &subscriptions {
        let mut resource_groups = vec![];

        let ip_addrs = service.get_ip_addresses(&subscription.subscription_id)?;

        for resource_group in service.get_resource_groups(&subscription.subscription_id)? {
            let mut ip_addresses = vec![];
            for ip in &ip_addrs {
                if ip.resource_group()? == resource_group.name {
                    ip_addresses.push(ip.clone());
                }
            }

            if !ip_addresses.is_empty() {
                resource_groups.push(IpResultResourceGroup {
                    resource_group,
                    ip_addresses,
                });
            }
        }

        if !resource_groups.is_empty() {
            result.push(IpResult {
                subscription: subscription.clone(),
                resource_groups,
            })
        }
    }

    return Ok(result);
}

#[derive(Serialize)]
pub struct CostResult {
    pub subscription: Subscription,
    pub costs: Vec<Costs>,
}

pub fn costs(context: &Context, timeframe: &Timeframe) -> Result<Vec<CostResult>> {
    let mut result = vec![];

    let service = &context.service;
    let subscriptions = service.get_subscriptions()?;
    for subscription in &subscriptions {
        let costs = service
            .get_costs(&subscription.subscription_id, timeframe)
            .unwrap_or(vec![]);
        result.push(CostResult {
            subscription: subscription.clone(),
            costs,
        });
    }

    return Ok(result);
}

pub fn get(context: &Context, request: &str) -> Result<Value> {
    return context.service.get(request, "");
}

pub fn post(context: &Context, request: &str, body: &str) -> Result<Value> {
    return context.service.post(request, "", body);
}
