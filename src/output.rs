use serde_json::to_string;
use serde_json::to_string_pretty;
use serde_json::Value;

use crate::commands::CostResult;
use crate::commands::DnsResult;
use crate::commands::Domain;
use crate::commands::IpResult;
use crate::commands::ListResult;
use crate::object::DnsRecordEntry;
use crate::object::Identifiable;
use crate::utils::Result;

pub trait Output {
    fn print_list_results(&self, results: &Vec<ListResult>) -> Result<()>;

    fn print_domains(&self, domains: &Vec<Domain>) -> Result<()>;

    fn print_dns_results(&self, results: &Vec<DnsResult>) -> Result<()>;

    fn print_ip_results(&self, results: &Vec<IpResult>) -> Result<()>;

    fn print_cost_results(&self, results: &Vec<CostResult>) -> Result<()>;

    fn print_value(&self, value: &Value) -> Result<()>;
}

pub struct JsonOutput {}

impl Output for JsonOutput {
    fn print_list_results(&self, results: &Vec<ListResult>) -> Result<()> {
        println!("{}", to_string_pretty(results)?);
        return Ok(());
    }

    fn print_domains(&self, domains: &Vec<Domain>) -> Result<()> {
        println!("{}", to_string_pretty(domains)?);
        return Ok(());
    }

    fn print_dns_results(&self, results: &Vec<DnsResult>) -> Result<()> {
        println!("{}", to_string_pretty(results)?);
        return Ok(());
    }

    fn print_ip_results(&self, results: &Vec<IpResult>) -> Result<()> {
        println!("{}", to_string_pretty(results)?);
        return Ok(());
    }

    fn print_cost_results(&self, results: &Vec<CostResult>) -> Result<()> {
        println!("{}", to_string_pretty(results)?);
        return Ok(());
    }

    fn print_value(&self, value: &Value) -> Result<()> {
        println!("{}", to_string_pretty(value)?);
        return Ok(());
    }
}

pub struct TextOutput {}

impl Output for TextOutput {
    fn print_list_results(&self, results: &Vec<ListResult>) -> Result<()> {
        for result in results {
            println!("{}", result.subscription.name);

            for resource_group in &result.resource_groups {
                println!("  {}", resource_group.name);

                for resource in &result.resources {
                    if resource.resource_group()? == resource_group.name {
                        println!("    {} ({})", resource.name, resource.resource_type);
                    }
                }
            }
        }

        return Ok(());
    }

    fn print_domains(&self, domains: &Vec<Domain>) -> Result<()> {
        for domain in domains {
            println!("{}", domain.name);

            let mut depth = 0;
            for entry in &domain.entries {
                match entry {
                    Some(DnsRecordEntry::CNAME(cname)) => {
                        println!("{0:1$} -> {2}", "", depth * 4, cname);
                        depth += 1;
                    }
                    None => println!("{0:1$} -> [recursion depth exceeded]", "", depth * 4),
                    _ => (),
                }
            }

            for ip_address in &domain.ip_addresses {
                println!("{0:1$} -> {2}", "", depth * 4, ip_address.ip_address);

                if let Some(resource_group) = ip_address.resource_group.as_ref() {
                    println!("{0:1$}     -> {2}", "", depth * 4, resource_group.name);
                }
            }
        }

        return Ok(());
    }

    fn print_dns_results(&self, results: &Vec<DnsResult>) -> Result<()> {
        for result in results {
            println!("{}", result.zone.name);

            for record in &result.records {
                println!("  {}", record.name);
                match &record.entry {
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

    fn print_ip_results(&self, results: &Vec<IpResult>) -> Result<()> {
        for result in results {
            println!("{}", result.subscription.name);

            for resource_group in &result.resource_groups {
                println!("  {}", resource_group.resource_group.name);

                for ip in &resource_group.ip_addresses {
                    println!("    {}", ip.ip_address);
                }
            }
        }

        return Ok(());
    }

    fn print_cost_results(&self, results: &Vec<CostResult>) -> Result<()> {
        let mut total = 0.0;
        let mut total_currency = None;

        for result in results {
            println!("{}", result.subscription.name);

            let mut sum = 0.0;
            let mut sum_currency = None;

            for item in &result.costs {
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

    fn print_value(&self, value: &Value) -> Result<()> {
        println!("{}", to_string(value)?);
        return Ok(());
    }
}
