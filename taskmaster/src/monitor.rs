pub mod processus;
pub mod program;
pub mod logger;
pub mod instruction;
pub mod parsing;

use std::error::Error;
use std::collections::HashMap;
use std::io;
use std::sync::mpsc::{Sender, Receiver};
use std::{thread, vec};
use std::time::Duration;
use std::path::PathBuf;
use std::process::ExitStatus;
use processus::{Status, Processus};
use logger::Logger;
use program::Program;
use parsing::Parsing;
use instruction::Instruction;

use crate::signal::Signal;
use crate::sys::Libc;

use self::processus::id::Id;

fn sig_handler(sig: i32) {
    println!("recieved sighup");
}

pub struct Monitor {
    config_file_path: PathBuf,
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
                processus.push(Processus::new(name, program));
            }
            i += program.config.numprocs;
        }
        
        Ok(Monitor {
            config_file_path: file_path.to_owned(),
            processus,
            logger,
            programs,
        })
    }

    pub fn execute(&mut self, receiver: Receiver<Instruction>, mut sender: Sender<Instruction>) {
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
                    Instruction::Restart(programs) => self.restart_command(programs, &mut sender),
                    Instruction::Reload(file_path) => self.reload(),
                    Instruction::Remove(ids) => unimplemented!(),
                    Instruction::StartProcessus(String, Id) => self.reload(),
                    Instruction::ResetProcessus(String, Id) => self.reload(),
                    Instruction::RetryStartProcessus(String, Id) => self.reload(),
                    Instruction::SetStatus(String, Id, String) => self.reload(),
                    Instruction::KillProcessus(String, Id) => self.reload(),
                    Instruction::Exit => self.stop_all(),
                }
            }
            self.monitor(sender.clone());
            thread::sleep(Duration::from_millis(300));
        }
    }
}

impl Monitor {

    fn monitor_active_processus(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>) -> Option<Instruction> {
        match exit_code {
            Some(code) => {
                if (program.config.autorestart == "unexpected"
                && program.config.exitcodes.iter().find(|&&e| e == code.code().expect("Failed to get exit code")) == None)
                || program.config.autorestart == "true" {
                    return Some(Instruction::StartProcessus(processus.name, processus.id))
                } else {
                    return Some(Instruction::ResetProcessus(processus.name, processus.id))
                }
            },
            _ => {return None},
        }
        
    }

    fn monitor_inactive_processus(processus: &Processus) {
        panic!("Child exist but the processus {} {} status is Inactive", processus.id, processus.name);
    }

    fn monitor_starting_processus(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>) -> Option<Instruction> {
        match exit_code {
            Some(code) => {
                if (program.config.autorestart == "true")
                || (program.config.autorestart == "unexpected"
                && program.config.exitcodes.iter().find(|&&e| e == code.code().expect("Failed to get exit code")) == None) {
                    return Some(Instruction::RetryStartProcessus(processus.name, processus.id))
                } else {
                    return Some(Instruction::ResetProcessus(processus.name, processus.id))
                }
            },
            None => {
                if processus.is_timeout(program.config.starttime) {
                    return Some(Instruction::SetStatus(processus.name, processus.id, "Active".to_string()))
                }
                None
            },
        }
    }

    fn monitor_stoping_processus(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>) -> Option<Instruction> {
        match exit_code {
            Some(code) => {
                return Some(Instruction::ResetProcessus(processus.name, processus.id))
            },
            None => {
                if processus.is_timeout(program.config.stoptime) {
                    return Some(Instruction::KillProcessus(processus.name, processus.id))
                }
                return None
            },
        }
    }

    fn monitor_processus(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>) -> Option<Instruction> {
        match processus.status {
            Status::Active => {return Self::monitor_active_processus(program, processus, exit_code)},
            Status::Inactive => {Self::monitor_inactive_processus(processus)},
            Status::Starting => {return Self::monitor_starting_processus(program, processus, exit_code)},
            Status::Stoping => {return Self::monitor_stoping_processus(program, processus, exit_code)},
            _ => {},
        };
        None
    }

    fn monitor_remove(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>) -> Option<Id> {
        match exit_code {
            Some(code) => {return Some(processus.id)},
            None => {
                if processus.is_timeout(program.config.stoptime) {
                    return Some(processus.id)
                }
            },
        }
        None
    }

    fn monitor(&mut self, sender: Sender<Instruction>) -> Vec<Instruction> {
        let instructions = Vec::new();
        let processus_to_remove = Vec::new();

        for processus in self.processus.iter() {
            if let Some(child) = processus.child.as_mut() {
                match child.try_wait() {
                    Err(_) => panic!("Try_wait failed on processus {} {}", processus.id, processus.name),
                    e => {
                        if let Some(instruction) = Self::monitor_processus(self.programs.get(&processus.name).unwrap(), processus, e.unwrap()) {
                            if processus.status == Status::Remove {
                                if let Some(id) = Self::monitor_remove(self.programs.get(&processus.name).unwrap(), processus, e.unwrap()) {
                                    processus_to_remove.push(id);
                                }
                            } else {
                                instructions.push(instruction);
                            }
                        }
                    },
                };
            } else {
                match processus.status {
                    Status::Inactive => {},
                    _ => {
                        panic!("Status is set but processus {} {} has no child", processus.name, processus.id);
                    },
                }
            }
        }
        if processus_to_remove.len() > 0 {
            instructions.push(Instruction::Remove(processus_to_remove))
        }
        instructions
    }

    fn status_command(&mut self) {
        println!("{:-<55}", "-");
        println!("| {:^5} | {:^20} | {:^20} |", "ID", "NAME", "STATUS");
        println!("{:-<55}", "-");
        for proc in self.processus.iter_mut() {
                println!("| {:^5} | {:^20} | {:^20} |", proc.id, proc.name.chars().take(20).collect::<String>(), proc.status);
        }
        println!("{:-<55}", "-");
        self.logger.log("Displaying Status");
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
                if processus.status == Status::Inactive {
                    Monitor::start_processus(processus, program);
                }
            }
            self.logger.log(&format!("Starting {}", &name));
        }
    }

    fn start_processus(processus: &mut Processus, program: &mut Program) {
        if let Some(child) = processus.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    if let Err(err) = processus.start_child(program.command.as_mut().unwrap(), program.config.startretries, program.config.umask) {
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
            if let Err(err) = processus.start_child(program.command.as_mut().unwrap(), program.config.startretries, program.config.umask) {
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
            self.logger.log(&format!("Stoping {}", &name));
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

    fn restart_command(&mut self, names: Vec<String>, sender: &mut Sender<Instruction>) {
        for name in &names {
            match self.programs.get(name) {
                None => {
                    eprintln!("{} program not found", name);
                    return ;
            },
                _ => {},
            }
        }

        self.logger.log("Restarting");
        self.stop_command(names.to_owned());
        
        for name in names.to_owned() {
            let duration = Duration::new(self.programs.get(&name).expect("program not found").config.stoptime as u64, 0);
            let sender = sender.clone();
            thread::spawn(move || {
                thread::sleep(duration);
                sender.send(Instruction::Start(vec!(name)));
            });
        }
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
        let new_programs = match Parsing::parse(&self.config_file_path) {
            Ok(programs) => programs,
            Err(err) => {
                self.logger.log(&format!("Failed to reload config file: {}", err));
                return;
            }
        };
        // 1. If some programs disapeared we stop the concerned procs and do not track them anymore
        let mut to_stop: Vec<String> = Vec::new();
        for (name, _) in self.programs.iter_mut().filter(|e| !new_programs.contains_key(e.0)) {
            to_stop.push(name.to_owned());
            self.processus.retain(|e| &e.name != name);
        }
        self.stop_command(to_stop);
        for (name, mut program) in new_programs {
            if self.programs.contains_key(&name) {
                // 2. Check all progs and if the conf hasn't changed do nothing
                if self.programs.iter().filter(|e| e.0 == &name).next().unwrap().1.config == program.config {
                    continue;
                } else {
                    // 3. If something has changed then restart the procs with the new config
                    if let Err(err) = program.build_command() {
                        eprintln!("Program {}: {}", name, err.to_string());
                        continue;
                    }
                    self.stop_command(vec!(name.to_owned()));
                    self.processus.retain(|e| &e.name != &name);
                    for id in 0..program.config.numprocs {
                        self.processus.push(Processus::new(&name, &program));
                    }
                    self.programs.insert(name, program);
                }
            } else {
                // 4. If some new programs appeared we start tracking them and start if necessery
                if let Err(err) = program.build_command() {
                    eprintln!("Program {}: {}", name, err.to_string());
                    continue;
                }
                for id in 0..program.config.numprocs {
                    self.processus.push(Processus::new(&name, &program));
                }
                self.programs.insert(name, program);
            }
        }
        // 4. If some programs disapeared we stop the concerned procs and do not track them anymore
        self.logger.log("Reloading config file");
    }
}