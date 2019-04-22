use serde_json::to_string_pretty;
use serde_json::Value;

use crate::service::Service;
use crate::utils::Result;

pub struct Context {
    pub service: Service,
}

pub fn get(context: &Context, request: &str) -> Result<()> {
    let result: Value = context.service.get(request, "")?;
    println!("{}", to_string_pretty(&result)?);
    return Ok(());
}

pub fn list(context: &Context, list_resources: bool) -> Result<()> {
    let service = &context.service;

    let subscriptions = service.get_subscriptions()?;
    for subscription in subscriptions {
        println!("{}", subscription.display_name);

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
