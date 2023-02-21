use std::{process, io};
use taskmaster::Taskmaster;

fn main() {
    let mut taskmaster: Taskmaster = Taskmaster::build("simple.conf").unwrap_or_else(|err| {
        eprintln!("Taskmaster: {err}");
        process::exit(1);
    });
    taskmaster.execute().unwrap_or_else(|err| {
        eprintln!("Taskmaster: {err}");
        process::exit(1);
    });
}
