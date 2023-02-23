use std::{process, io};
use taskmaster::*;

fn main() {
    let mut taskmaster: Taskmaster = Taskmaster::new("simple.conf").unwrap_or_else(|err| {
        eprintln!("Taskmaster: {err}");
        process::exit(1);
    });
    let mut monitor = Monitor::new();
    taskmaster.execute(monitor).unwrap_or_else(|err| {
        eprintln!("Taskmaster: {err}");
        process::exit(1);
    });
}
