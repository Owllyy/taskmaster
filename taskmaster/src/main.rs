use std::process;
mod file;
use taskmaster::Taskmaster;

fn main() {

    let taskmaster = Taskmaster::new(file::get_config_from_args()).unwrap_or_else(|err| {
        eprintln!("Taskmaster: {err}");
        process::exit(1);
    });
    taskmaster.execute().unwrap_or_else(|err| {
        eprintln!("Taskmaster: {err}");
        process::exit(1);
    });
}
