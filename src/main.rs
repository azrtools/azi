#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

mod auth;
mod cli;
mod client;
mod commands;
mod error;
mod http;
mod object;
mod output;
mod service;
mod tenant;
mod utils;

use cli::run;

fn main() {
    run();
}
