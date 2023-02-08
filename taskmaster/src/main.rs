use std::process;
use std::process::Command;
use crate::parsing_config::Config;

pub mod parsing_config;

fn main() {
    let config = Config::build("template.conf").unwrap_or_else(|err| {
        eprintln!("Problem parsing configuration file: {err}");
        process::exit(1);
    });
}
