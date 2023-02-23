pub mod monitor;
pub mod signal;

use monitor::*;
use monitor::work_queue::*;
use std::io::{self};
use std::{process, thread};
use std::error::Error;

#[allow(non_camel_case_types)]
type mode_t = u32;

extern "C" {
    fn umask(mask: mode_t) -> mode_t;
    fn kill(pid: i32, sig: i32) -> i32;
    fn signal(
        signum: i32, 
        handler: usize,
    ) -> usize;
}

pub const SIG_IGN: usize = 1;

pub struct Taskmaster {
    monitor: Monitor,
    config_file_path: String,
    work_queue: WorkQueue,
}

impl Taskmaster {
    pub fn new(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let work_queue = WorkQueue::new();
        
        Ok(Taskmaster {
            monitor: Monitor::new(work_queue.clone(), file_path)?,
            config_file_path: file_path.to_string(),
            work_queue,
        })
    }

    pub fn execute(& mut self, mut monitor: Monitor) -> Result<(), Box<dyn Error>> {
        thread::spawn(move || {
            monitor.execute();
        });
        self.cli();
        Ok(())
    }

    fn cli(&mut self) {
        let mut buff = String::new();
        loop {
            io::stdin().read_line(&mut buff).expect("Failed to read");
            let input: Vec<&str> = buff.split_whitespace().collect();
            if let Some(instruct) = input.get(0) {
                match instruct.to_lowercase().as_str() {
                    "exit" => {
                        process::exit(0);
                    }
                    "status" => {
                        self.work_queue.push(Instruction::Status);
                    }
                    "start" => {
                        if let Some(arg) = input.get(1) {
                            self.work_queue.push(Instruction::Start(vec![arg.to_string()]));
                        } else {
                            println!("Which program you want to start ? ($ start nginx)");
                        }
                    }
                    "stop" => {
                        if let Some(arg) = input.get(1) {
                            self.work_queue.push(Instruction::Stop(vec![arg.to_string()]));
                        } else {
                            println!("Which program you want to stop ? ($ stop nginx)");
                        }
                    }
                    "restart" => {
                        if let Some(arg) = input.get(1) {
                            self.work_queue.push(Instruction::Restart(vec![arg.to_string()]));
                        } else {
                            println!("Which program you want to restart ? ($ restart nginx)");
                        }
                    }
                    _ => {
                        println!("Unknown command");
                    }
                }
            }
            buff.clear();
        }
    }
}