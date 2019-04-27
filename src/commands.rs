use serde_json::to_string_pretty;
use serde_json::Value;

use crate::object::DnsRecordEntry;
use crate::object::Identifiable;
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

pub fn dns(context: &Context) -> Result<()> {
    let service = &context.service;

    let subscriptions = service.get_subscriptions()?;

    let mut zones = vec![];
    for subscription in &subscriptions {
        zones.extend(service.get_resources_by_type(&subscription.subscription_id, TYPE_DNS_ZONE)?);
    }

    for zone in &zones {
        println!("{}", zone.name);

        let records = service.get_dns_records(
            zone.subscription_id().unwrap(),
            zone.resource_group().unwrap(),
            &zone.name,
        )?;

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

        let groups = service.get_resource_groups(&subscription.subscription_id)?;
        for group in groups {
            println!("  {}", group.name);

            let ips = service.get_ip_addresses(&subscription.subscription_id, &group.name)?;
            for ip in ips {
                println!("    {}", ip.properties.ip_address);
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
