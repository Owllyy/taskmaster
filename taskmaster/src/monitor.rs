pub mod processus;
pub mod program;
pub mod logger;
pub mod instruction;
pub mod parsing;

use std::error::Error;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
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
use crate::sys::{Libc, self};

use self::processus::id::Id;

const INACTIVE_FLAG: &str = "Inactive";

fn sig_handler(sig: i32) {
    sys::RELOAD_INSTRUCTION.store(true, Ordering::SeqCst);
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

        for (name, program) in programs.iter_mut() {
            if let Err(err) = program.build_command() {
                eprintln!("Program {}: {}", name, err.to_string());
                continue;
            }
            for _ in 0..program.config.numprocs {
                processus.push(Processus::new(name, program));
            }
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

        let mut instruction_queue: VecDeque<Instruction> = VecDeque::new();
        
        loop {
            if sys::RELOAD_INSTRUCTION.load(Ordering::SeqCst) {
                instruction_queue.push_front(Instruction::Reload);
                sys::RELOAD_INSTRUCTION.store(false, Ordering::SeqCst);
            }
            if let Ok(instruction) = receiver.try_recv() {
                instruction_queue.push_back(instruction);
            }
            while let Some(instruction) = instruction_queue.pop_front() {
                match instruction {
                    // Instruction from cli
                    Instruction::Status => self.status_command(),
                    Instruction::Start(programs) => self.start_command(programs),
                    Instruction::Stop(programs) => self.stop_command(programs),
                    Instruction::Restart(programs) => self.restart_command(programs, &mut sender),
                    Instruction::Reload => self.reload(),
                    // Instruction not from Cli
                    Instruction::RemoveProcessus(id, is_remove) => self.remove_processus(id, is_remove),
                    Instruction::StartProcessus(id) => self.start_processus(id),
                    Instruction::ResetProcessus(id) => self.reset_processus(id),
                    Instruction::RetryStartProcessus(id) => self.start_processus(id),
                    Instruction::SetStatus(id, status) => self.set_status(id, status),
                    Instruction::KillProcessus(id) => self.kill_processus(id),
                    Instruction::Exit => self.stop_all(),
                }
            }
            let mut iteration_instructions: VecDeque<Instruction> = VecDeque::new();
            iteration_instructions.extend(self.monitor());
            instruction_queue.append(&mut iteration_instructions);
            thread::sleep(Duration::from_millis(300));
        }
    }
}

impl Monitor {
    fn get_processus(processus: &mut Vec<Processus>, id: Id) -> Option<&mut Processus> {
        processus.iter_mut().find(|processus| processus.id == id)
    }

    fn kill_processus(&mut self, id: Id) {
        let processus = Self::get_processus(&mut self.processus, id);

        if let Some(processus) = processus{
            if let Some(child) = &mut processus.child {
                child.kill();
            }
            if processus.status != Status::Reloading(true | false) {
                processus.status = Status::Inactive;
            }
            self.logger.log(&format!("Sigkill processus {} {}", processus.name, processus.id));
        }
    }

    fn set_status(&mut self, id: Id, status: Status) {
        if let Some(processus) = Self::get_processus(&mut self.processus, id) {
            processus.status = status;
            self.logger.log(&format!("Seting status of processus {} {} to Active", processus.name, processus.id));
        }
    }

    fn start_processus(&mut self, id: Id) {
        if let Some(processus) = Self::get_processus(&mut self.processus, id) {
            if let Some(program) = self.programs.get_mut(&processus.name) {
                if let Some(command) = &mut program.command {
                    match processus.start_child(command, program.config.startretries, program.config.umask) {
                        Ok(false) => {self.logger.log(&format!("Starting processus {} {}, {} atempt left", processus.name, processus.id, processus.retries));},
                        Ok(true) => {self.logger.log(&format!("Failed to start processus {} {}, no atempt left", processus.name, processus.id));},
                        Err(err) => {eprintln!("{:?}", err);self.logger.log(&format!("{:?}", err));},
                    } 
                } else {
                    eprintln!("Can't find command to start processus {} {}", processus.name, processus.id);
                }
            } else {
                eprintln!("Can't find program to start processus {} {}", processus.name, processus.id);
            }
        }
    }

    fn reset_processus(&mut self, id: Id) {
        if let Some(processus) = Self::get_processus(&mut self.processus, id) {
            if let Some(program) = self.programs.get(&processus.name) {
                self.logger.log(&format!("Reset processus {} {}", processus.name, processus.id));
                processus.reset_child(program.config.startretries)
            }
        }
    }

    // Need rework logic problem
    fn remove_processus(&mut self, id: Id, is_remove: bool) {
        if let Some(processus) = Self::get_processus(&mut self.processus, id) {
            let processus_name = processus.name.to_owned();
            self.processus.retain(|proc| proc.id != id);
            if self.processus.iter().filter(|e| e.name == processus_name).collect::<Vec<&Processus>>().len() == 0 {
                // remove old one anyway
                self.programs.remove(&processus_name);
                // if there is with inactive flag do stuff
                let name = if let Some((name, _)) = self.programs.iter().find(|e| e.0 == &[INACTIVE_FLAG, &processus_name].concat()) {
                    name.to_owned()
                } else {
                    return;
                };
                if let Some(mut program) = self.programs.remove(&name) {
                    program.activate();
                    self.programs.insert(processus_name.to_owned(), program);
                    let program = self.programs.get(&processus_name).unwrap();
                    for _ in 0..program.config.numprocs {
                        self.processus.push(Processus::new(&processus_name, program));
                    }
                    if program.config.autostart {
                        self.start_command(vec![processus_name]);
                    }
                }
            }
        }
    }

    fn monitor_active_processus(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>) -> Option<Instruction> {
        match exit_code {
            Some(code) => {
                if (program.config.autorestart == "unexpected"
                && program.config.exitcodes.iter().find(|&&e| e == code.code().expect("Failed to get exit code")) == None)
                || program.config.autorestart == "true" {
                    return Some(Instruction::StartProcessus(processus.id))
                } else {
                    return Some(Instruction::ResetProcessus(processus.id))
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
                if ((program.config.autorestart == "true")
                || (program.config.autorestart == "unexpected"
                && program.config.exitcodes.iter().find(|&&e| e == code.code().expect("Failed to get exit code")) == None)) && processus.retries > 0 {
                    Some(Instruction::RetryStartProcessus(processus.id))
                } else {
                    Some(Instruction::ResetProcessus(processus.id))
                }
            },
            None => {
                if processus.is_timeout(program.config.starttime) {
                    Some(Instruction::SetStatus(processus.id, Status::Active))
                } else {
                    None
                }
            },
        }
    }

    fn monitor_stoping_processus(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>) -> Option<Instruction> {
        match exit_code {
            Some(_) => Some(Instruction::ResetProcessus(processus.id)),
            None => {
                if processus.is_timeout(program.config.stoptime) {
                    Some(Instruction::KillProcessus(processus.id))
                } else {
                    None
                }
            }
        }
    }

    fn monitor_remove_processus(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>, is_remove: bool) -> Option<Instruction> {
        match exit_code {
            Some(_) => {
                Some(Instruction::RemoveProcessus(processus.id, is_remove))
            }
            None => {
                if processus.is_timeout(program.config.stoptime) {
                    Some(Instruction::KillProcessus(processus.id))
                } else {
                    None
                }
            }
        }
    }

    fn monitor_processus(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>) -> Option<Instruction> {
        let tmp = match processus.status {
            Status::Active => Self::monitor_active_processus(program, processus, exit_code),
            Status::Inactive => {Self::monitor_inactive_processus(processus); None},
            Status::Starting => Self::monitor_starting_processus(program, processus, exit_code),
            Status::Stoping => Self::monitor_stoping_processus(program, processus, exit_code),
            Status::Reloading(is_remove) => Self::monitor_remove_processus(program, processus, exit_code, is_remove),
        };
        tmp
    }

    fn monitor(&mut self) -> Vec<Instruction> {
        let mut instructions = Vec::new();

        for processus in self.processus.iter_mut() {
            if let Some(child) = processus.child.as_mut() {
                match child.try_wait() {
                    Err(_) => panic!("Try_wait failed on processus {} {}", processus.id, processus.name),
                    e => {
                        if let Some(instruction) = Self::monitor_processus(self.programs.get(&processus.name).unwrap(), processus, e.unwrap()) {
                            instructions.push(instruction);
                        }
                    },
                };
            }
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
            self.logger.log(&format!("Starting program {}", &name));
            if let None = self.programs.get_mut(&name) {
                eprintln!("Program not found: {}", name);
                continue;
            };
            let filtered_processus_ids: Vec<Id> = self.processus.iter().filter_map(|e| {
                if e.name == name && e.status == Status::Inactive {
                    Some(e.id)
                } else {
                    None
                }
            }).collect();
            for pid in filtered_processus_ids {
                self.start_processus(pid);
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
                Self::stop_processus(processus, program);
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
                    if let Err(err) = processus.stop_child(program.config.stopsignal, program.config.startretries) {
                        eprintln!("{}", err.to_string());
                    }
                }
                Err(_) => {
                    panic!("try_wait() failed");
                },
            };
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

        self.stop_command(names.to_owned());
        
        for name in names.to_owned() {
            self.logger.log(&format!("Restarting {}", name));
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
                self.logger.log(&format!("Autostart {}", name));
                to_start.push(name.to_owned());
            }
        }
        self.start_command(to_start);
    }

    fn stop_all(&mut self) {
        // let mut to_stop = Vec::new();
        // for (name, _) in self.programs.iter() {
        //     to_stop.push(name.to_owned());
        // }
        // self.stop_command(to_stop);
        // while let Some(proc) = self.processus.iter().find(|e| e.child.is_some()) {
        //     for instruction in self.monitor() {
        //         match instruction {
        //             Instruction::ResetProcessus(id) => self.reset_processus(id),
        //             Instruction::KillProcessus(id) => self.kill_processus(id),
        //         }
        //     }
        // }
    }
    
    fn reload(&mut self) {
        self.logger.log("Reloading config file");
        let new_programs = match Parsing::parse(&self.config_file_path) {
            Ok(programs) => programs,
            Err(err) => {
                self.logger.log(&format!("Failed to reload config file: {}", err));
                return;
            }
        };
        // 1. If some programs disapeared we stop the concerned procs and do not track them anymore
        for (name, _) in self.programs.iter_mut().filter(|e| !new_programs.contains_key(e.0)) {
            for proc in self.processus.iter_mut().filter(|e| &e.name == name) {
                proc.status = Status::Reloading(true);
            }
        }
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
                    for proc in self.processus.iter_mut().filter(|e| e.name == name) {
                        proc.status = Status::Reloading(false);
                    }
                    program.deactivate();
                    self.programs.insert(Program::prefix_name(INACTIVE_FLAG, name), program);
                }
            } else {
                // 4. If some new programs appeared we start tracking them and start if necessery
                if let Err(err) = program.build_command() {
                    eprintln!("Program {}: {}", name, err.to_string());
                    continue;
                }
                for _ in 0..program.config.numprocs {
                    self.processus.push(Processus::new(&name, &program));
                }
                self.programs.insert(name.to_owned(), program);
                let program = self.programs.get(&name).unwrap();
                if program.config.autostart {
                    self.start_command(vec![name]);
                }
            }
        }
    }
}
