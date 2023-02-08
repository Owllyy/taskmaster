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
    
    let mut command = String::new();
    loop {
        io::stdin().read_line(& mut command).expect("Failed to read input");
        match command.as_str() {
            "status" => check_status(),
            "help" => display_commands(),
            _ => println!("Unknown command, type \"help\" to show all commands"),
        }
    }
}
