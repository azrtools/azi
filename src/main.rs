extern crate dirs;
extern crate env_logger;
extern crate regex;
extern crate reqwest;
extern crate serde_derive;
extern crate serde_json;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

mod auth;
mod cli;
mod client;
mod commands;
mod error;
mod object;
mod service;
mod utils;

use cli::run;
use client::Client;
use commands::Context;
use service::Service;

fn main() {
    let client = Client::new();
    let service = Service::new(client);

    let context = Context { service };

    run(&context);
}
