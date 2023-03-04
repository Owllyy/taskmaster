pub mod monitor;
pub mod signal;
mod sys;

use monitor::*;
use monitor::instruction::*;
use std::io::{self};
use std::sync::mpsc::{self, Sender};
use std::{process, thread};
use std::error::Error;
use std::path::PathBuf;

pub struct Taskmaster {
    config_file_path: PathBuf,
}

impl Taskmaster {
    pub fn new(file_path: PathBuf) -> Result<Self, Box<dyn Error>> {
        Ok(Taskmaster {
            config_file_path: file_path,
        })
    }

    pub fn execute(mut self) -> Result<(), Box<dyn Error>> {
        let (sender, receiver) = mpsc::channel::<Instruction>();
        let mut sender_clone = sender.clone();
        let mut monitor = Monitor::new(&self.config_file_path)?;
        thread::spawn(move || {
            monitor.execute(receiver, sender_clone);
        });
        self.cli(sender);
        Ok(())
    }

    fn cli(&mut self, sender: Sender<Instruction>) {
        let mut buff = String::new();
        loop {
            buff.clear();
            io::stdin().read_line(&mut buff).expect("Failed to read");
            let instruction: Instruction = match buff.parse() {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("{}", e.to_string());
                    continue;
                }
            };
            // Todo rework Exit instruction
            if let Instruction::Exit = instruction {
                process::exit(0);
            }
            if let Err(_) = sender.send(instruction) {
                eprintln!("Failed to execute instruction");
            }
        }
    }
}