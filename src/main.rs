extern crate dirs;
extern crate env_logger;
extern crate regex;
extern crate reqwest;
extern crate serde_derive;
extern crate serde_json;

#[macro_use]
extern crate log;

#[macro_use]
extern crate clap;

use std::error::Error;

use clap::App;
use clap::Arg;

use log::LevelFilter;

mod auth;
mod client;
mod error;
mod object;
mod service;

use client::Client;
use service::Service;

fn main() {
    let matches = App::new("azi")
        .version(crate_version!())
        .about("Show Azure information")
        .usage("azi [-h] [-v] [--debug] [--trace]")
        .template("usage: {usage}\n\n{about}\n\nOptional arguments:\n{flags}")
        .version_short("v")
        .arg(Arg::with_name("debug").short("d").help("Show debug output"))
        .arg(Arg::with_name("trace").short("t").help("Show trace output"))
        .get_matches();

    let mut logger = env_logger::Builder::new();
    if matches.is_present("trace") {
        logger.filter(Some("azi"), LevelFilter::Trace);
    } else if matches.is_present("debug") {
        logger.filter(Some("azi"), LevelFilter::Debug);
    } else {
        logger.filter(Some("azi"), LevelFilter::Info);
    };
    logger.init();

    let client = Client::new();
    let service = Service::new(client);

    match run(&service) {
        Ok(_) => return,
        Err(e) => println!("Error! {:#?}", e),
    }
}

fn run(service: &Service) -> Result<(), Box<Error>> {
    let subscriptions = service.get_subscriptions()?;
    for subscription in subscriptions {
        println!("{}", subscription.display_name);

        let groups = service.get_resource_groups(&subscription.subscription_id)?;
        for group in groups {
            println!("  {}", group.name);
        }
    }

    return Ok(());
}
