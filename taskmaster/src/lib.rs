use std::{fs};
use std::fs::File;
use std::io::{self};
use std::process::{self, Command, Stdio};
use serde::Deserialize;
use std::error::Error;
use std::collections::HashMap;
use std::thread::{self};
use std::sync::{Mutex, Arc};

pub mod processus;
use processus::{Status, Processus};

#[allow(non_camel_case_types)]
type mode_t = u32;

extern "C" {
    fn umask(mask: mode_t) -> mode_t;
}

struct Logger {
    output: Box<dyn io::Write>,
}

impl Default for Logger {
    fn default() -> Self {
        Self {
            output: Box::new(io::stdout()),
        }
    }
}

impl Logger {
    fn new(file_path: &str) -> Self {
        let output: Box<dyn io::Write> = match File::create(file_path) {
            Ok(file) => Box::new(file),
            Err(_) => Box::new(io::stdout()),
        };
        Self {
            output
        }
    }

    fn log(&mut self, msg: &str) {
        self.output.write(msg.as_bytes()).expect("Failed to log");
        self.output.write(&[b'\n']).expect("Failed to log");
    }
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Task {
    cmd: String,
    numprocs: usize,
    umask: String,
    workingdir: String,
    autostart: bool,
    autorestart: String,
    exitcodes: Vec<i32>,
    startretries: usize,
    starttime: usize,
    stopsignal: String,
    stoptime: usize,
    stdout: String,
    stderr: String,
    env: HashMap<String, String>,
    #[serde(skip)]
    command: Option<Command>,
}

#[derive(Deserialize)]
struct Config {
    #[serde(flatten)]
    config: HashMap<String, Task>,
}

pub enum Instruction {
    Status,
    Start(Vec<String>),
    Stop(Vec<String>),
    Restart(Vec<String>)
}

pub struct Taskmaster {
    processus: Arc<Mutex<Vec<Processus>>>,
    logger: Logger,
    config: Arc<Mutex<HashMap<String, Task>>>,
    workQueue: Arc<Mutex<Vec<Instruction>>>,
}

impl Taskmaster {

    fn executioner(work_q: &Arc<Mutex<Vec<Instruction>>>, procs: &Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>) {
        let work_q = Arc::clone(&work_q);
        let mut procs = Arc::clone(&procs);
        let config = Arc::clone(&config);
        thread::spawn(move || {
            loop {
                if let Some(instruction) = work_q.lock().expect("Mutex Lock failed").pop() {
                    match instruction {
                        Instruction::Status => Taskmaster::status_command(&mut procs),
                        Instruction::Start(task) => Taskmaster::start_command(&mut procs, &config, task),
                        Instruction::Stop(task) => Taskmaster::stop_command(&mut procs, &config, task),
                        Instruction::Restart(task) => Taskmaster::restart_command(&mut procs, &config, task),
                    }
                }
                Taskmaster::monitor(&procs, &config);
            }
        });
    }

    fn monitor(procs: &Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>) {
        let mut procs = procs.lock().expect("Fail to lock Mutex");
        let mut config = config.lock().expect("Fail to lock Mutex");
        for (name, task) in config.iter_mut() {
            for proc in procs.iter_mut().filter(|e| &e.name == name) {
                if let Some(child) = proc.child.as_mut() {
                    match child.try_wait() {
                        Ok(Some(exitcode)) => {
                            match proc.status {
                                Status::Active => {
                                    //todo understand the double ref &&
                                    if (task.autorestart == "unexpected" && task.exitcodes.iter().find(|e| e == &&exitcode.code().expect("Failed to get exit code")) == None)
                                    || task.autorestart == "true" {
                                        proc.start_child(task.command.as_mut().unwrap(), task.startretries);
                                    } else {
                                        proc.reset_child();
                                    }
                                },
                                Status::Inactive => {
                                    panic!("Child exist but the status is Inactive");
                                },
                                Status::Starting => {
                                    if (task.autorestart == "true")
                                    || (task.autorestart == "unexpected" && task.exitcodes.iter().find(|e| e == &&exitcode.code().expect("Failed to get exit code")) == None) {
                                        proc.child = Some(task.command.as_mut().expect("Command is not build").spawn().expect("Spawn failed"));
                                        proc.retries -= 1;
                                        proc.set_timer();
                                    } else {
                                        proc.reset_child();
                                    }
                                },
                                Status::Stoping => {
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
                                    if proc.check_timer(task.starttime) {
                                        proc.status = Status::Active;
                                    }
                                },
                                Status::Stoping => {
                                    if proc.check_timer(task.stoptime) {
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

    fn status_command(procs: &mut Arc<Mutex<Vec<Processus>>>) {
        let mut procs = procs.lock().expect("Fail to lock Mutex");
        println!("{:-<55}", "-");
        println!("| {:^5} | {:^20} | {:^20} |", "ID", "NAME", "STATUS");
        println!("{:-<55}", "-");
        for proc in procs.iter_mut() {
                println!("| {:^5} | {:^20} | {:^20} |", proc.id, proc.name.chars().take(20).collect::<String>(), proc.status);
        }
        println!("{:-<55}", "-");
    }

    fn start_command(procs: &Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>, names: Vec<String>) {
        let mut procs = procs.lock().expect("Fail to lock Mutex");
        let mut config = config.lock().expect("Fail to lock Mutex");

        for name in names {
            let task = if let Some(task) = config.get_mut(&name) {
                task
            } else {
                println!("Command not found: {name}");
                break;
            };
            for proc in procs.iter_mut().filter(|e| e.name == name) {
                Taskmaster::start_processus(proc, task);
            }
        }
    }

    fn start_processus(proc: &mut Processus, task: &mut Task) {
        if let Some(child) = proc.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    proc.start_child(task.command.as_mut().unwrap(), task.startretries);
                },
                Ok(None) => {
                    println!("The program {} is already running", proc.name);
                }
                Err(_) => {
                    panic!("try_wait() failed");
                },
            };
        } else {
            proc.start_child(task.command.as_mut().unwrap(), task.startretries);
        }
    }

    fn stop_command(procs: &mut Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>, names: Vec<String>) {
        let mut procs = procs.lock().expect("Fail to lock Mutex");
        let mut config = config.lock().expect("Fail to lock Mutex");
        for name in names {
            let task = if let Some(task) = config.get_mut(&name) {
                task
            } else {
                println!("Unknown Program");
                return;
            };
            for proc in procs.iter_mut().filter(|e| e.name == name) {
                Taskmaster::stop_processus(proc, task);
            }
        }
    }

    fn stop_processus(proc: &mut Processus, task: &mut Task) {
        if let Some(child) = proc.child.as_mut() {
            match child.try_wait() {
                Ok(Some(exitstatus)) => {
                    println!("The program {} as stoped running, exit code : {exitstatus}", proc.name);
                },
                Ok(None) => {
                    proc.stop_child(&task.stopsignal);
                }
                Err(_) => {
                    panic!("try_wait() failed");
                },
            };
        } else {
            println!("The program {} is not running", proc.name);
        }
    }

    fn restart_command(procs: &mut Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>, names: Vec<String>) {
        Taskmaster::stop_command(procs, config, names.to_owned());
        Taskmaster::start_command(procs, config, names);
    }

    fn start_all_autostart_task(procs: &Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>) {
        let mut all_task: Vec<String> = Vec::new();
        for (name, task) in config.lock().expect("Mutex lock failed").iter() {
            if task.autostart {
                all_task.push(name.to_owned());
            }
        }
        Taskmaster::start_command(procs, config, all_task);
    }

    fn stop_all_task(procs: &mut Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>) {
        let mut all_task = Vec::new();
        for (name, _) in config.lock().expect("Mutex lock failed").iter() {
            all_task.push(name.to_owned());
        }
        Taskmaster::stop_command(procs, config, all_task);
    }

    pub fn build(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let commands: Arc<Mutex<Vec<Processus>>> = Arc::new(Mutex::new(vec!()));
        let config = fs::read_to_string(file_path)?;
        let config: Config = serde_yaml::from_str(&config)?;
        let mut logger = Logger::new("taskmaster.log");
        logger.log("Configuration file successfully parsed");
        
        Ok(Taskmaster {
            processus: commands,
            logger,
            config: Arc::new(Mutex::new(config.config)),
            workQueue: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn execute(& mut self) -> Result<(), Box<dyn Error>> {
        let mut i = 0;
        self.processus = Arc::new(Mutex::new(Vec::<Processus>::new()));

        self.logger.log("Building all processus...");
        for (name, properties) in self.config.lock().expect("Mutex lock failed").iter_mut() {
            
            Self::build_command(properties)?;
            
            let mut lock = self.processus.lock().expect("Mutex lock failed");
            for id in 0..properties.numprocs {
                lock.push(Processus::build(i + id, name, properties.startretries));
            }
            i += properties.numprocs;
        }
        
        self.logger.log("Starting all 'autostart' processuses...");
        Taskmaster::start_all_autostart_task(&self.processus, &self.config);
        self.logger.log("Launching executioner...");
        Taskmaster::executioner(&self.workQueue, &self.processus, &self.config);
        self.cli();
        Ok(())
    }

    fn build_command(task: &mut Task) -> Result<(), Box<dyn Error>> {
        let cmd: Vec<&str> = task.cmd.split_whitespace().collect();
        let output = Self::create_output(&task)?;
        task.command = Some(Command::new(cmd.get(0).unwrap()));

        task.command.as_mut().unwrap().args(cmd.get(1).iter())
            .envs(task.env.iter())
            .current_dir(&task.workingdir)
            .stdout(output.0)
            .stderr(output.1);
        
        Ok(())
    }

    fn create_output(task: &Task) -> Result<(Stdio, Stdio), Box<dyn Error>> {
        let stdout = Stdio::from(File::open(&task.stdout).unwrap_or(File::create(&task.stdout)?));
        let stderr = Stdio::from(File::open(&task.stderr).unwrap_or(File::create(&task.stderr)?));

        Ok((stdout, stderr))
    }

    fn cli(&mut self) {
        let mut buff = String::new();
        self.logger.log("Staring the CLI");
        loop {
            io::stdin().read_line(&mut buff).expect("Failed to read");
            let input: Vec<&str> = buff.split_whitespace().collect();
            if let Some(instruct) = input.get(0) {
                match instruct.to_lowercase().as_str() {
                    "exit" => {
                        process::exit(0);
                    }
                    "status" => {
                        let mut queue = self.workQueue.lock().expect("Mutex Lock failed");
                        queue.push(Instruction::Status);
                    }
                    "start" => {
                        if let Some(arg) = input.get(1) {
                            let mut queue = self.workQueue.lock().expect("Mutex Lock failed");
                            queue.push(Instruction::Start(vec![arg.to_string()]));
                        } else {
                            println!("Which program you want to start ? ($ start nginx)");
                        }
                    }
                    "stop" => {
                        if let Some(arg) = input.get(1) {
                            let mut queue = self.workQueue.lock().expect("Mutex Lock failed");
                            queue.push(Instruction::Stop(vec![arg.to_string()]));
                        } else {
                            println!("Which program you want to stop ? ($ stop nginx)");
                        }
                    }
                    "restart" => {
                        if let Some(arg) = input.get(1) {
                            let mut queue = self.workQueue.lock().expect("Mutex Lock failed");
                            queue.push(Instruction::Restart(vec![arg.to_string()]));
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