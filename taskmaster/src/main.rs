use std::{process, io};
use taskmaster::Taskmaster;

fn check_status() {
    println!("Status check");
}

fn display_commands() {
    println!("Help");
}

fn main() {
    let mut taskmaster: Taskmaster = Taskmaster::build("template.conf").unwrap_or_else(|err| {
        eprintln!("Taskmaster: {err}");
        process::exit(1);
    });
    taskmaster.execute();
    //taskmaster.monitor();
}
