pub mod processus;
pub mod program;
pub mod logger;
pub mod instruction;
pub mod parsing;

use std::error::Error;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Sender, Receiver};
use std::{thread, vec, process};
use std::time::Duration;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::os::unix::process::ExitStatusExt;
use processus::{Status, Processus};
use logger::Logger;
use program::Program;
use parsing::Parsing;
use instruction::Instruction;

use crate::signal::{Signal};
use crate::sys::{Libc, self};

use self::processus::id::Id;

const INACTIVE_FLAG: &str = "Inactive";

fn sig_handler(_: i32) {
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

        let mut invalid_confs = Vec::<String>::new();
        for (name, program) in programs.iter_mut() {
            if let Err(err) = program.build_command() {
                eprintln!("Program {name}: {err}");
                invalid_confs.push(name.to_owned());
                continue;
            }
            for _ in 0..program.config.numprocs {
                processus.push(Processus::new(name, program));
            }
        }
        for name in &invalid_confs {
            programs.remove(name);
        }
        
        Ok(Monitor {
            config_file_path: file_path.to_owned(),
            processus,
            logger,
            programs,
        })
    }

    pub fn execute(&mut self, receiver: Receiver<Instruction>, mut sender: Sender<Instruction>) {
        if Libc::signal(Signal::SIGHUP, sig_handler).is_err() {
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
                    Instruction::RemoveProcessus(id) => self.remove_processus(id),
                    Instruction::StartProcessus(id) => self.start_processus(id, false),
                    Instruction::ResetProcessus(id) => self.reset_processus(id),
                    Instruction::RetryStartProcessus(id) => self.start_processus(id, true),
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

    fn get_processus(processus: &mut [Processus], id: Id) -> Option<&mut Processus> {
        processus.iter_mut().find(|processus| processus.id == id)
    }

    fn kill_processus(&mut self, id: Id) {
        let processus = Self::get_processus(&mut self.processus, id);

        if let Some(processus) = processus{
            if let Some(child) = &mut processus.child {
                child.kill().ok();
            }
            processus.child = None;
            if processus.status != Status::Reloading {
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

    fn start_processus(&mut self, id: Id, restart: bool) {
        if let Some(processus) = Self::get_processus(&mut self.processus, id) {
            if let Some(program) = self.programs.get_mut(&processus.name) {
                if let Some(command) = &mut program.command {
                    match processus.start_child(command, program.config.startretries, program.config.umask, restart) {
                        Ok(false) => {self.logger.log(&format!("Starting processus {} {}, {} atempt left", processus.name, processus.id, processus.retries));},
                        Ok(true) => {self.logger.log(&format!("Failed to start processus {} {}, no atempt left", processus.name, processus.id));},
                        Err(err) => {eprintln!("{err:?}");self.logger.log(&format!("{err:?}"));},
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

    fn remove_processus(&mut self, id: Id) {
        if let Some(processus) = Self::get_processus(&mut self.processus, id) {
            let processus_name = processus.name.to_owned();
            self.processus.retain(|proc| proc.id != id);
            if self.processus.iter().filter(|e| e.name == processus_name).collect::<Vec<&Processus>>().is_empty() {
                self.programs.remove(&processus_name);
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
                match program.config.autorestart.as_str() {
                    "always" => {Some(Instruction::StartProcessus(processus.id))},
                    "never" => {Some(Instruction::ResetProcessus(processus.id))},
                    "unexpected" => {
                        let is_normal_exit_code = program.config.exitcodes.iter().find(|&&e| e == code.code().expect("Failed to get exit code"));
                        if is_normal_exit_code.is_none() {
                            Some(Instruction::StartProcessus(processus.id))
                        } else {
                            Some(Instruction::ResetProcessus(processus.id))
                        }
                    },
                    _ => {panic!("autorestart has an invalid value");}
                }
            },
            _ => {None},
        }
    }

    fn monitor_inactive_processus(processus: &Processus) {
        panic!("Child exist but the processus {} {} status is Inactive", processus.id, processus.name);
    }

    fn monitor_starting_processus(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>) -> Option<Instruction> {
        match exit_code {
            Some(_) => {
                if processus.retries > 0 {
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

    fn monitor_remove_processus(program: &Program, processus: &Processus, exit_code: Option<ExitStatus>) -> Option<Instruction> {
        match exit_code {
            Some(_) => {
                Some(Instruction::RemoveProcessus(processus.id))
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
        match processus.status {
            Status::Active => Self::monitor_active_processus(program, processus, exit_code),
            Status::Inactive => {Self::monitor_inactive_processus(processus); None},
            Status::Starting => Self::monitor_starting_processus(program, processus, exit_code),
            Status::Stoping => Self::monitor_stoping_processus(program, processus, exit_code),
            Status::Reloading => Self::monitor_remove_processus(program, processus, exit_code),
        }
    }

    fn monitor(&mut self) -> Vec<Instruction> {
        let mut instructions = Vec::new();

        for processus in self.processus.iter_mut() {
            if let Some(child) = processus.child.as_mut() {
                match child.try_wait() {
                    Err(_) => panic!("Try_wait failed on processus {} {}", processus.id, processus.name),
                    Ok(code) => {
                        if let Some(code) = code {
                            if let Some(signal) = code.signal() {
                                if processus.status != Status::Reloading {
                                    self.logger.log(&format!("Processus {} {} was stopped by a signal: {}", processus.name, processus.id, signal));
                                    instructions.push(Instruction::ResetProcessus(processus.id));
                                    continue;
                                }
                            }
                        }
                        if let Some(instruction) = Self::monitor_processus(self.programs.get(&processus.name).unwrap(), processus, code) {
                            instructions.push(instruction);
                        }
                    },
                };
            } else if processus.status == Status::Reloading {
                instructions.push(Instruction::RemoveProcessus(processus.id));
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
            if self.programs.get_mut(&name).is_none() {
                eprintln!("Program not found: {name}");
                continue;
            }
            let filtered_processus_ids: Vec<Id> = self.processus.iter().filter_map(|e| {
                if e.name == name && e.status == Status::Inactive {
                    Some(e.id)
                } else {
                    None
                }
            }).collect();
            for pid in filtered_processus_ids {
                self.start_processus(pid, false);
            }
        }
    }

    fn stop_command(&mut self, names: Vec<String>) {
        for name in names {
            let program = if let Some(program) = self.programs.get_mut(&name) {
                program
            } else {
                eprintln!("Program not found: {name}");
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
                        eprintln!("{err}");
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
            if self.programs.get(name).is_none() {
                eprintln!("Program not found: {name}");
                return ;
            }
        }

        self.stop_command(names.to_owned());
        
        for name in names {
            self.logger.log(&format!("Restarting {name}"));
            let duration = Duration::new(self.programs.get(&name).expect("program not found").config.stoptime as u64, 0);
            let sender = sender.clone();
            thread::spawn(move || {
                thread::sleep(duration);
                sender.send(Instruction::Start(vec!(name))).ok();
            });
        }
    }

    fn autostart(&mut self) {
        let mut to_start: Vec<String> = Vec::new();
        for (name, program) in self.programs.iter() {
            if program.config.autostart {
                self.logger.log(&format!("Autostart {name}"));
                to_start.push(name.to_owned());
            }
        }
        self.start_command(to_start);
    }

    fn stop_all(&mut self) {
        let mut to_stop = Vec::new();
        self.logger.log("Shutting down taskmaster");
        for (name, _) in self.programs.iter() {
            to_stop.push(name.to_owned());
        }
        self.stop_command(to_stop);
        while self.processus.iter().any(|e| e.child.is_some()) {
            for instruction in self.monitor() {
                match instruction {
                    Instruction::ResetProcessus(id) => self.reset_processus(id),
                    Instruction::KillProcessus(id) => self.kill_processus(id),
                    _ => {}
                }
            }
        }
        process::exit(0);
    }
    
    fn reload(&mut self) {
        self.logger.log("Reloading config file");
        let new_programs = match Parsing::parse(&self.config_file_path) {
            Ok(programs) => programs,
            Err(err) => {
                self.logger.log(&format!("Failed to reload config file: {err}"));
                return;
            }
        };
        // 1. If some programs disapeared we stop the concerned procs and do not track them anymore
        let to_remove: Vec<String> = self.programs.iter_mut().filter_map(|e| {
            if !new_programs.contains_key(e.0) {
                Some(e.0.to_owned())
            } else {
                None
            }
        }).collect();
        self.stop_command(to_remove.to_owned());
        for name in &to_remove {
            for proc in self.processus.iter_mut().filter(|e| &e.name == name) {
                proc.status = Status::Reloading;
            }
        }
        for (name, mut program) in new_programs {
            if self.programs.contains_key(&name) {
                // 2. Check all progs and if the conf hasn't changed do nothing
                if self.programs.iter().find(|e| e.0 == &name).unwrap().1.config == program.config {
                    continue;
                } else {
                    // 3. If something has changed then restart the procs with the new config
                    if let Err(err) = program.build_command() {
                        eprintln!("Program {name}: {err}");
                        continue;
                    }
                    self.stop_command(vec!(name.to_owned()));
                    for proc in self.processus.iter_mut().filter(|e| e.name == name) {
                        proc.status = Status::Reloading;
                    }
                    program.deactivate();
                    self.programs.insert(Program::prefix_name(INACTIVE_FLAG, name), program);
                }
            } else {
                // 4. If some new programs appeared we start tracking them and start if necessery
                if let Err(err) = program.build_command() {
                    eprintln!("Program {name}: {err}");
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
