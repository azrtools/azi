use std::collections::HashMap;

use serde_json::to_string_pretty;
use serde_json::Value;

use crate::object::DnsRecord;
use crate::object::DnsRecordEntry;
use crate::object::Identifiable;
use crate::object::ResourceGroup;
use crate::service::Service;
use crate::service::Timeframe;
use crate::service::TYPE_DNS_ZONE;
use crate::utils::Result;

pub struct Context {
    pub service: Service,
}

pub fn list(context: &Context, list_resources: bool) -> Result<()> {
    let service = &context.service;

    let subscriptions = service.get_subscriptions()?;
    for subscription in subscriptions {
        println!("{}", subscription.name);

        let groups = service.get_resource_groups(&subscription.subscription_id)?;
        for group in groups {
            println!("  {}", group.name);
        }

        if list_resources {
            let resources = service.get_resources(&subscription.subscription_id)?;
            for resource in resources {
                println!(
                    "    {} {}",
                    resource.kind.unwrap_or("unknown".to_string()),
                    resource.name
                );
            }
        }
    }

    return Ok(());
}

pub fn domains(context: &Context, filter: Option<&String>) -> Result<()> {
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
            let group_name = ip.resource_group()?;
            let group = groups.iter().find(|group| group.name == group_name);
            if let Some(group) = group {
                ip_to_group.insert(ip.ip_address, group.clone());
            }
        }
    }

    let mut domains: Vec<&String> = (&records).iter().map(|record| &record.fqdn).collect();

    if let Some(filter) = filter {
        domains.retain(|domain| domain.contains(filter));
    } else {
        for record in &records {
            match &record.entry {
                DnsRecordEntry::CNAME(cname) => {
                    domains.retain(|&domain| domain != cname);
                }
                _ => (),
            }
        }
    }

    domains.sort();

    const MAX_DEPTH: usize = 5;

    fn find_target<'f>(
        records: &'f Vec<DnsRecord>,
        domain: &str,
        depth: usize,
    ) -> Option<(&'f DnsRecord, usize)> {
        for record in records {
            if &record.fqdn == domain {
                match &record.entry {
                    DnsRecordEntry::CNAME(cname) => {
                        if depth >= MAX_DEPTH {
                            println!("{0:1$} -> [recursion depth exceeded]", "", depth * 4);
                            return None;
                        } else {
                            println!("{0:1$} -> {2}", "", depth * 4, cname);
                            return find_target(records, cname, depth + 1);
                        }
                    }
                    DnsRecordEntry::A(_) => {
                        return Some((record, depth));
                    }
                }
            }
        }
        return None;
    }

    for domain in &domains {
        println!("{}", domain);

        if let Some((target, depth)) = find_target(&records, domain, 0) {
            match &target.entry {
                DnsRecordEntry::A(ip_addresses) => {
                    for ip_address in ip_addresses {
                        println!("{0:1$} -> {2}", "", depth * 4, ip_address);

                        if let Some(group) = ip_to_group.get(ip_address) {
                            println!("{0:1$}     -> {2}", "", depth * 4, group.name);
                        }
                    }
                }
                _ => (),
            }
        }
    }

    return Ok(());
}

pub fn dns(context: &Context) -> Result<()> {
    let service = &context.service;

    let subscriptions = service.get_subscriptions()?;

    let mut zones = vec![];
    for subscription in &subscriptions {
        zones.extend(service.get_resources_by_type(&subscription.subscription_id, TYPE_DNS_ZONE)?);
    }

    for zone in &zones {
        println!("{}", zone.name);

        let records =
            service.get_dns_records(zone.subscription_id()?, zone.resource_group()?, &zone.name)?;

        for record in records {
            println!("  {}", record.name);
            match record.entry {
                DnsRecordEntry::A(ip_addresses) => {
                    for ip in ip_addresses {
                        println!("    A {}", ip);
                    }
                }
                DnsRecordEntry::CNAME(cname) => println!("    CNAME {}", cname),
            }
        }
    }

    return Ok(());
}

pub fn ip(context: &Context) -> Result<()> {
    let service = &context.service;

    let subscriptions = service.get_subscriptions()?;

    for subscription in &subscriptions {
        println!("{}", subscription.name);

        let ips = service.get_ip_addresses(&subscription.subscription_id)?;

        let groups = service.get_resource_groups(&subscription.subscription_id)?;
        for group in groups {
            println!("  {}", group.name);

            for ip in &ips {
                if ip.resource_group()? == group.name {
                    println!("    {}", ip.ip_address);
                }
            }
        }
    }

    return Ok(());
}

pub fn costs(context: &Context, timeframe: &Timeframe) -> Result<()> {
    let service = &context.service;

    let subscriptions = service.get_subscriptions()?;

    let mut total = 0.0;
    let mut total_currency = None;

    for subscription in &subscriptions {
        println!("{}", subscription.name);

        let mut sum = 0.0;
        let mut sum_currency = None;

        let costs = service.get_costs(&subscription.subscription_id, timeframe)?;
        for item in &costs {
            println!(
                "  {}  {:0.2} {}",
                item.resource_group, item.costs, item.currency
            );
            sum += item.costs;
            if sum_currency == None {
                sum_currency = Some(&item.currency);
            }
        }

        if let Some(currency) = sum_currency {
            println!("  sum  {:0.2} {}", sum, currency);
            total += sum;
            total_currency = Some(currency.clone());
        }
    }

    if let Some(currency) = total_currency {
        println!("total  {:0.2} {}", total, currency);
    }

    return Ok(());
}

pub fn get(context: &Context, request: &str) -> Result<()> {
    let result: Value = context.service.get(request, "")?;
    println!("{}", to_string_pretty(&result)?);
    return Ok(());
}

pub fn post(context: &Context, request: &str, body: &str) -> Result<()> {
    let result: Value = context.service.post(request, "", body)?;
    println!("{}", to_string_pretty(&result)?);
    return Ok(());
}
