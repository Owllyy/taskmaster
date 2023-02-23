pub mod processus;
pub mod program;
pub mod logger;
pub mod work_queue;
pub mod parsing;

use std::error::Error;
use std::collections::HashMap;
use std::thread::{self};
use processus::{Status, Processus};
use logger::Logger;
use program::Program;
use parsing::Parsing;
use work_queue::{WorkQueue, Instruction};

use crate::signal::{self, c_int, Signal, SIG_IGN};

#[allow(non_camel_case_types)]
type mode_t = u32;

fn sig_handler(sig: c_int) {
    println!("recieved sighup");
}

pub struct Monitor {
    processus: Vec<Processus>,
    logger: Logger,
    programs: HashMap<String, Program>,
    work_queue: WorkQueue,
}

impl Monitor {
    pub fn new(work_queue: WorkQueue, file_path: &str) -> Result<Self, Box<dyn Error>> {
        let mut programs = Parsing::parse(file_path)?;
        let logger = Logger::new("taskmaster.log");
        let mut processus: Vec<Processus> = Vec::new();

        let mut i = 0;

        for (name, program) in programs.iter_mut() {
            program.build_command();
            
            for id in 0..program.config.numprocs {
                processus.push(Processus::new(i + id, name, program.config.startretries));
            }
            i += program.config.numprocs;
        }
        
        Ok(Monitor {
            processus,
            logger,
            programs,
            work_queue,
        })
    }

    pub fn execute(&mut self) {
        unsafe {
            signal::signal(Signal::SIGHUP as i32, sig_handler as usize);
            signal::signal(Signal::SIGUSR1 as i32, SIG_IGN);
            signal::signal(Signal::SIGUSR2 as i32, SIG_IGN);
        }
        self.autostart();
        loop {
            if let Some(instruction) = self.work_queue.pop() {
                match instruction {
                    Instruction::Status => self.status_command(),
                    Instruction::Start(programs) => self.start_command(programs),
                    Instruction::Stop(programs) => self.stop_command(programs),
                    Instruction::Restart(programs) => self.restart_command(programs),
                    Instruction::Reload(file_path) => self.reload(),
                }
            }
            self.monitor();
        }
    }
}

impl Monitor {

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
                                        proc.start_child(program.command.as_mut().unwrap(), program.config.startretries, program.config.umask.parse::<mode_t>().expect("umask is in wrong format"));
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
                                        proc.set_timer();
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
                                    if proc.check_timer(program.config.starttime) {
                                        proc.status = Status::Active;
                                    }
                                },
                                Status::Stoping => {
                                    if proc.check_timer(program.config.stoptime) {
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

    fn status_command(&self) {
        println!("{:-<55}", "-");
        println!("| {:^5} | {:^20} | {:^20} |", "ID", "NAME", "STATUS");
        println!("{:-<55}", "-");
        for proc in self.processus.iter_mut() {
                println!("| {:^5} | {:^20} | {:^20} |", proc.id, proc.name.chars().take(20).collect::<String>(), proc.status);
        }
        println!("{:-<55}", "-");
    }

    fn start_command(&self, names: Vec<String>) {
        for name in names {
            let program = if let Some(program) = self.programs.get_mut(&name) {
                program
            } else {
                println!("Command not found: {name}");
                break;
            };
            for processus in self.processus.iter_mut().filter(|e| e.name == name) {
                Monitor::start_processus(processus, program);
            }
        }
    }

    fn start_processus(processus: &mut Processus, program: &mut Program) {
        if let Some(child) = processus.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    processus.start_child(program.command.as_mut().unwrap(), program.config.startretries, program.config.umask.parse::<mode_t>().expect("umask is in wrong format"));
                },
                Ok(None) => {
                    println!("The program {} is already running", processus.name);
                }
                Err(_) => {
                    panic!("try_wait() failed");
                },
            };
        } else {
            processus.start_child(program.command.as_mut().unwrap(), program.config.startretries, program.config.umask.parse::<mode_t>().expect("umask is in wrong format"));
        }
    }

    fn stop_command(&self, names: Vec<String>) {
        for name in names {
            let program = if let Some(program) = self.programs.get_mut(&name) {
                program
            } else {
                println!("Unknown Program");
                return;
            };
            for processus in self.processus.iter_mut().filter(|e| e.name == name) {
                Monitor::stop_processus(processus, program);
            }
        }
    }

    fn stop_processus(processus: &mut Processus, program: &mut Program) {
        if let Some(child) = processus.child.as_mut() {
            match child.try_wait() {
                Ok(Some(exitstatus)) => {
                    println!("The program {} as stoped running, exit code : {exitstatus}", processus.name);
                },
                Ok(None) => {
                    processus.stop_child(&program.config.stopsignal);
                }
                Err(_) => {
                    panic!("try_wait() failed");
                },
            };
        } else {
            println!("The program {} is not running", processus.name);
        }
    }

    fn restart_command(&self, names: Vec<String>) {
        //todo rework with a thread waiting the "Stoping" time to push the start_command
        self.stop_command(names.to_owned());
        self.start_command(names);
    }

    fn autostart(&self) {
        let mut to_start: Vec<String> = Vec::new();
        for (name, program) in self.programs.iter() {
            if program.config.autostart {
                to_start.push(name.to_owned());
            }
        }
        self.start_command(to_start);
    }

    fn stop_all(&self) {
        let mut to_stop = Vec::new();
        for (name, _) in self.programs.iter() {
            to_stop.push(name.to_owned());
        }
        self.stop_command(to_stop);
    }

    fn reload(&self) {

    }
}