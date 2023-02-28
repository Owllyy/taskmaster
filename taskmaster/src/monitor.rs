pub mod processus;
pub mod program;
pub mod logger;
pub mod instruction;
pub mod parsing;

use std::error::Error;
use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;
use std::path::PathBuf;
use processus::{Status, Processus};
use logger::Logger;
use program::Program;
use parsing::Parsing;
use instruction::Instruction;

use crate::signal::Signal;
use crate::sys::Libc;

fn sig_handler(sig: i32) {
    println!("recieved sighup");
}

pub struct Monitor {
    processus: Vec<Processus>,
    logger: Logger,
    programs: HashMap<String, Program>,
}

impl Monitor {
    pub fn new(file_path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let mut programs = Parsing::parse(file_path)?;
        let logger = Logger::new("taskmaster.log")?;
        let mut processus: Vec<Processus> = Vec::new();

        let mut i = 0;

        for (name, program) in programs.iter_mut() {
            if let Err(err) = program.build_command() {
                eprintln!("Program {}: {}", name, err.to_string());
                continue;
            }
            
            for id in 0..program.config.numprocs {
                processus.push(Processus::new(i + id, name, program.config.startretries));
            }
            i += program.config.numprocs;
        }
        
        Ok(Monitor {
            processus,
            logger,
            programs,
        })
    }

    pub fn execute(&mut self, receiver: Receiver<Instruction>) {
        if let Err(_) = Libc::signal(Signal::SIGHUP, sig_handler) {
            eprintln!("Signal function failed, taskmaster won't be able to handle SIGHUP");
        }
        self.autostart();
        loop {
            if let Ok(instruction) = receiver.try_recv() {
                match instruction {
                    Instruction::Status => self.status_command(),
                    Instruction::Start(programs) => self.start_command(programs),
                    Instruction::Stop(programs) => self.stop_command(programs),
                    Instruction::Restart(programs) => self.restart_command(programs),
                    Instruction::Reload(file_path) => self.reload(),
                    Instruction::Exit => self.stop_all(),
                }
            }
            self.monitor();
            thread::sleep(Duration::from_millis(300));
        }
    }
}

impl Monitor {

    fn log(&mut self, msg: &str, name: Option<&String>) {
        let mut message = msg.to_string();
        match name {
            Some(name) => {message += " ";message += &name},
            None => {},
        }
        match self.logger.log(&message) {
            Ok(_) => {},
            Err(e) => {eprintln!("Logger : {e}")},
        }
    }

    fn monitor(&mut self) {
        for (name, program) in self.programs.iter_mut() {
            for proc in self.processus.iter_mut().filter(|e| &e.name == name) {
                if let Some(child) = proc.child.as_mut() {
                    match child.try_wait() {
                        Ok(Some(exitcode)) => {
                            match proc.status {
                                Status::Active => {
                                    //todo understand the double ref &&
                                    if (program.config.autorestart == "unexpected" && program.config.exitcodes.iter().find(|e| e == &&exitcode.code().expect("Failed to get exit code")) == None)
                                    || program.config.autorestart == "true" {
                                        if let Err(err) = proc.start_child(program.command.as_mut().unwrap(), program.config.startretries, program.config.umask.parse::<u32>().expect("umask is in wrong format")) {
                                            eprintln!("{}", err.to_string());
                                        }
                                    } else {
                                        proc.reset_child();
                                    }
                                },
                                Status::Inactive => {
                                    panic!("Child exist but the status is Inactive");
                                },
                                Status::Starting => {
                                    if (program.config.autorestart == "true")
                                    || (program.config.autorestart == "unexpected" && program.config.exitcodes.iter().find(|e| e == &&exitcode.code().expect("Failed to get exit code")) == None) {
                                        // maybe call the start proc function
                                        proc.child = Some(program.command.as_mut().expect("Command is not build").spawn().expect("Spawn failed"));
                                        proc.retries -= 1;
                                        proc.start_timer();
                                    } else {
                                        proc.reset_child();
                                    }
                                },
                                Status::Stoping => {
                                    // Donno if this is good
                                    proc.reset_child();
                                },
                            }
                        },
                        Ok(None) => {
                            match proc.status {
                                Status::Inactive => {
                                    panic!("The procesus is active but got the status Inactive");
                                },
                                Status::Starting => {
                                    if proc.is_timeout(program.config.starttime) {
                                        proc.status = Status::Active;
                                    }
                                },
                                Status::Stoping => {
                                    if proc.is_timeout(program.config.stoptime) {
                                        proc.child.as_mut().expect("No child but status is Stoping").kill().expect("Failed to kill child");
                                        proc.child = None;
                                        proc.status = Status::Inactive;
                                    }
                                },
                                _ => {},
                            }
                        },
                        Err(_) => {
                            panic!("try wait failed");
                        },
                    };
                } else {
                    //debug
                    match proc.status {
                        Status::Inactive => {},
                        _ => {
                            panic!("Status is set but there is no child");
                        },
                    }
                }
            }
        }
    }

    fn status_command(&mut self) {
        println!("{:-<55}", "-");
        println!("| {:^5} | {:^20} | {:^20} |", "ID", "NAME", "STATUS");
        println!("{:-<55}", "-");
        for proc in self.processus.iter_mut() {
                println!("| {:^5} | {:^20} | {:^20} |", proc.id, proc.name.chars().take(20).collect::<String>(), proc.status);
        }
        println!("{:-<55}", "-");
        self.log("Displaying Status", None);
    }

    fn start_command(&mut self, names: Vec<String>) {
        for name in names {
            let program = if let Some(program) = self.programs.get_mut(&name) {
                program
            } else {
                println!("Program not found: {name}");
                continue;
            };
            for processus in self.processus.iter_mut().filter(|e| e.name == name) {
                Monitor::start_processus(processus, program);
            }
            self.log("Starting", Some(&name));
        }
    }

    fn start_processus(processus: &mut Processus, program: &mut Program) {
        if let Some(child) = processus.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    if let Err(err) = processus.start_child(program.command.as_mut().unwrap(), program.config.startretries, program.config.umask.parse::<u32>().expect("umask is in wrong format")) {
                        eprintln!("{}", err.to_string());
                    }
                },
                Ok(None) => {
                    println!("The program {} is already running", processus.name);
                }
                Err(_) => {
                    panic!("try_wait() failed");
                },
            };
        } else {
            // Need to do the umask transformation and verification once
            if let Err(err) = processus.start_child(program.command.as_mut().unwrap(), program.config.startretries, program.config.umask.parse::<u32>().expect("umask is in wrong format")) {
                eprintln!("{}", err.to_string());
            }
        }
    }

    fn stop_command(&mut self, names: Vec<String>) {
        for name in names {
            let program = if let Some(program) = self.programs.get_mut(&name) {
                program
            } else {
                println!("Unknown Program");
                continue;
            };
            for processus in self.processus.iter_mut().filter(|e| e.name == name) {
                Monitor::stop_processus(processus, program);
            }
            self.log("Stoping", Some(&name));
        }
    }

    fn stop_processus(processus: &mut Processus, program: &mut Program) {
        if let Some(child) = processus.child.as_mut() {
            match child.try_wait() {
                Ok(Some(exitstatus)) => {
                    println!("The program {} as stoped running, exit code : {exitstatus}", processus.name);
                },
                Ok(None) => {
                    if let Err(err) = processus.stop_child(program.config.stopsignal) {
                        eprintln!("{}", err.to_string());
                    }
                }
                Err(_) => {
                    panic!("try_wait() failed");
                },
            };
        } else {
            println!("The program {} is not running", processus.name);
        }
    }

    fn restart_command(&mut self, names: Vec<String>) {
        self.log("Restarting", None);
        // todo rework with a thread waiting the "Stoping" time to push the start_command
        self.stop_command(names.to_owned());
        self.start_command(names);
    }

    fn autostart(&mut self) {
        let mut to_start: Vec<String> = Vec::new();
        for (name, program) in self.programs.iter() {
            if program.config.autostart {
                to_start.push(name.to_owned());
            }
        }
        self.start_command(to_start);
    }

    fn stop_all(&mut self) {
        let mut to_stop = Vec::new();
        for (name, _) in self.programs.iter() {
            to_stop.push(name.to_owned());
        }
        self.stop_command(to_stop);
    }

    fn reload(&mut self) {
        self.log("Reloading config file", None);
    }
}