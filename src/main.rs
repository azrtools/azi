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
mod output;
mod service;
mod utils;

use cli::run;

fn main() {
    run();
}
