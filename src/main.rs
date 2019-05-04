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
