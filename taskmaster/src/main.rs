use std::process;
use taskmaster::Taskmaster;

fn main() {
    let taskmaster = Taskmaster::new("simple.conf").unwrap_or_else(|err| {
        eprintln!("Taskmaster: {err}");
        process::exit(1);
    });
    taskmaster.execute().unwrap_or_else(|err| {
        eprintln!("Taskmaster: {err}");
        process::exit(1);
    });
}
