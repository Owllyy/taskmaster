use std::fmt::format;
use std::{fs, default};
use std::fs::File;
use std::io::{self, Read};
use std::process::{self, Command, Child, Stdio};
use serde::Deserialize;
use std::error::Error;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::thread::{self, sleep};
use std::sync::{Mutex, Arc};

#[allow(non_camel_case_types)]
type mode_t = u32;

extern "C" {
    fn umask(mask: mode_t) -> mode_t;
}

#[derive(Debug)]
pub struct Processus {
    id: usize,
    name: String,
    child: Option<Child>,
    retries: usize,
    timer: Instant,
}

impl Processus {
    fn build(id: usize, name: &str, retries: usize) -> Self {
        Self {
            id,
            name: name.to_owned(),
            child: None,
            retries,
            timer: Instant::now(),
        }
    }

    fn set_timer(&mut self) {
        self.timer = Instant::now();
    }

    fn check_timer(&self, duration: usize) -> bool {
        let duration = Duration::from_secs(duration as u64);
        
        if self.timer.elapsed() < duration {
            return false;
        }

        true
    }
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
    exitcodes: Vec<u8>,
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
    procs: Arc<Mutex<Vec<Processus>>>,
    logger: Logger,
    config: Arc<Mutex<HashMap<String, Task>>>,
    work_q: Arc<Mutex<Vec<Instruction>>>,
}

impl Taskmaster {

    pub fn executioner(work_q: &Arc<Mutex<Vec<Instruction>>>, procs: &Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>) {
        let work_q = Arc::clone(&work_q);
        let mut procs = Arc::clone(&procs);
        let config = Arc::clone(&config);
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(500));
    
                if let Some(instruction) = work_q.lock().expect("Mutex Lock failed").pop() {
                    match instruction {
                        Instruction::Status => Taskmaster::status(&mut procs),
                        Instruction::Start(task) => Taskmaster::start(&procs, &config, task),
                        Instruction::Stop(task) => Taskmaster::stop(&mut procs, &config, task),
                        Instruction::Restart(task) => Taskmaster::restart(&mut procs, &config, task),
                    }
                }
            }
        });
    }

    fn status(procs: &mut Arc<Mutex<Vec<Processus>>>) {
        let mut procs = procs.lock().expect("Fail to lock Mutex");
        println!("{:-<55}", "-");
        println!("| {:^5} | {:^20} | {:^20} |", "ID", "NAME", "STATUS");
        println!("{:-<55}", "-");
        for proc in procs.iter_mut() {
            if let Some(child) = proc.child.as_mut() {
                let status = match child.try_wait() {
                    Ok(Some(st)) => format!("{st}"),
                    Ok(None) => "active".to_owned(),
                    Err(_) => "error".to_owned(),
                };
                println!("| {:^5} | {:^20} | {:^20} |", proc.id, proc.name.chars().take(20).collect::<String>(), status);
            } else {
                println!("| {:^5} | {:^20} | {:^20} |", proc.id, proc.name.chars().take(20).collect::<String>(), "inactive");
            }
        }
        println!("{:-<55}", "-");
    }

    fn start(procs: &Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>, names: Vec<String>) {
        let mut procs = procs.lock().expect("Fail to lock Mutex");
        let mut config = config.lock().expect("Fail to lock Mutex");
        for name in names {
            for proc in procs.iter_mut() {
                if let Some(child) = proc.child.as_mut() {
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            proc.child = Some(config.get_mut(&name)
                                .expect("Failed to get_mut")
                                .command.as_mut().unwrap()
                                .spawn().expect("Failed to spawn proc"));
                        },
                        Ok(None) => {
                            println!("The program is already running");
                        }
                        Err(_) => {
                            panic!("try wait failed");
                        },
                    };
                } else {
                    if let Some(task) = config.get_mut(&name) {
                        proc.child = Some(task.command.as_mut().expect("Can't spawn command").spawn().expect("Failed to spawn proc"));
                    } else {
                        println!("Unknown Program");
                    }
                }
            }
        }
    }

    fn stop(procs: &mut Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>, names: Vec<String>) {
        let mut procs = procs.lock().expect("Fail to lock Mutex");
        let mut config = config.lock().expect("Fail to lock Mutex");
        for name in names {
            for proc in procs.iter_mut() {
                if let Some(child) = proc.child.as_mut() {
                    match child.try_wait() {
                        Ok(Some(exitstatus)) => {
                            println!("The program {name} as stoped running, exit code : {exitstatus}");
                        },
                        Ok(None) => {
                            // May be illegal to use the Linux command Kill to send Signal to child process
                            // If not switch to LibC way of sending signal
                            match Command::new("kill")
                                // TODO: replace `TERM` to signal you want.
                                .args(["-s", &config.get(&name).unwrap().stopsignal, &child.id().to_string()])
                                .spawn() {
                                    Ok(_) => {},
                                    Err(e) => panic!("Fail to send the stop signal : {e}"),
                                }
                        }
                        Err(_) => {
                            panic!("try_wait() failed");
                        },
                    };
                } else {
                    if let Some(_) = config.get_mut(&name) {
                        println!("The program {name} is not running");
                    } else {
                        println!("Unknown Program");
                    }
                }
            }
        }
    }

    fn restart(procs: &mut Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>, names: Vec<String>) {
        Taskmaster::stop(procs, config, names.to_owned());
        Taskmaster::start(procs, config, names);
    }

    fn start_all(procs: &Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>) {
        let mut all_task: Vec<String> = Vec::new();
        for (name, task) in config.lock().expect("Mutex lock failed").iter() {
            if task.autostart {
                all_task.push(name.to_owned());
            }
        }
        Taskmaster::start(procs, config, all_task);
    }

    fn stop_all(procs: &mut Arc<Mutex<Vec<Processus>>>, config: &Arc<Mutex<HashMap<String, Task>>>) {
        let mut all_task = Vec::new();
        for (name, _) in config.lock().expect("Mutex lock failed").iter() {
            all_task.push(name.to_owned());
        }
        Taskmaster::stop(procs, config, all_task);
    }

    pub fn build(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let commands: Arc<Mutex<Vec<Processus>>> = Arc::new(Mutex::new(vec!()));
        let config = fs::read_to_string(file_path)?;
        let config: Config = serde_yaml::from_str(&config)?;
        let mut logger = Logger::new("taskmaster.log");
        logger.log("Configuration file successfully parsed");
        
        Ok(Taskmaster {
            procs: commands,
            logger,
            config: Arc::new(Mutex::new(config.config)),
            work_q: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn execute(& mut self) -> Result<(), Box<dyn Error>> {
        let mut i = 0;
        self.procs = Arc::new(Mutex::new(Vec::<Processus>::new()));

        self.logger.log("Building all processus...");
        for (name, properties) in self.config.lock().expect("Mutex lock failed").iter_mut() {
            
            Self::build_command(properties)?;
            
            let mut lock = self.procs.lock().expect("Mutex lock failed");
            for id in 0..properties.numprocs {
                lock.push(Processus::build(i + id, name, properties.startretries));
            }
            i += properties.numprocs;
        }
        
        self.logger.log("Starting all 'autostart' processuses...");
        Taskmaster::start_all(&self.procs, &self.config);
        self.logger.log("Launching executioner...");
        Taskmaster::executioner(&self.work_q, &self.procs, &self.config);
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
                        println!("STATUS");
                        let mut queue = self.work_q.lock().expect("Mutex Lock failed");
                        queue.push(Instruction::Status);
                    }
                    "start" => {
                        if let Some(arg) = input.get(1) {
                            let mut queue = self.work_q.lock().expect("Mutex Lock failed");
                            queue.push(Instruction::Start(vec![arg.to_string()]));
                        } else {
                            println!("Which program you want to start ? ($ start nginx)");
                        }
                    }
                    "stop" => {
                        if let Some(arg) = input.get(1) {
                            let mut queue = self.work_q.lock().expect("Mutex Lock failed");
                            queue.push(Instruction::Stop(vec![arg.to_string()]));
                        } else {
                            println!("Which program you want to stop ? ($ stop nginx)");
                        }
                    }
                    "restart" => {
                        if let Some(arg) = input.get(1) {
                            let mut queue = self.work_q.lock().expect("Mutex Lock failed");
                            queue.push(Instruction::Restart(vec![arg.to_string()]));
                        } else {
                            println!("Which program you want to restart ? ($ restart nginx)");
                        }
                    }
                    _ => {
                        println!("Unknown command");
                    }
                }
            } else {
                println!("wtf");
            }
            buff.clear();
        }
    }
}