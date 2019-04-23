use serde_json::to_string_pretty;
use serde_json::Value;

use crate::object::Identifiable;
use crate::service::Service;
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
            if record.record_type.ends_with("/A") || record.record_type.ends_with("/CNAME") {
                println!("  {}", record.name);

                if let Some(arecords) = record.properties.records {
                    for arecord in arecords {
                        println!("    A {}", arecord.ip_address);
                    }
                }

                if let Some(cname) = record.properties.cname {
                    println!("    CNAME {}", cname.cname);
                }
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

pub fn get(context: &Context, request: &str) -> Result<()> {
    let result: Value = context.service.get(request, "")?;
    println!("{}", to_string_pretty(&result)?);
    return Ok(());
}
