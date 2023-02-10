use std::fmt::format;
use std::{fs, default};
use std::fs::File;
use std::io::{self, Read};
use std::process::{self, Command, Child, Stdio};
use serde::Deserialize;
use std::error::Error;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::thread::sleep;
use std::sync::Mutex;

#[allow(non_camel_case_types)]
type mode_t = u32;

extern "C" {
    fn umask(mask: mode_t) -> mode_t;
}

#[derive(Debug)]
struct Processus {
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
    }
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields, default)]
struct Task {
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

enum Instruction {
    Status,
    Start(String),
    Stop(String),
    Restart(String)
}

#[derive(Deserialize)]
pub struct Taskmaster {
    #[serde(skip)]
    procs: Vec<Mutex<Processus>>,
    #[serde(skip)]
    logger: Logger,
    #[serde(flatten)]
    config: HashMap<String, Task>,
    #[serde(skip)]
    workQ: Mutex<Vec<Instruction>>,
}

impl Taskmaster {

    pub fn executioner(&self) -> Result<(), Box<dyn Error>> {
        loop {
            let mut queue = self.workQ.lock().expect("Mutex Lock failed");

            if let Some(instruction) = queue.pop() {
                match instruction {
                    Instruction::Status => self.status(),
                    Instruction::Start(task) => unimplemented!(),
                    Instruction::Stop(task) => unimplemented!(),
                    Instruction::Restart(task) => unimplemented!(),
                }
            }
        }
    }

    fn status(&self) {
        println!("{:-<55}", "-");
        println!("| {:^5} | {:^20} | {:^20} |", "ID", "NAME", "STATUS");
        println!("{:-<55}", "-");
        for processus in self.procs.iter() {
            let mut proc = processus.lock().expect("Mutex Lock failed");
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

    fn start(&mut self, name: String) {
        for proc in self.procs.iter() {
            let mut proc = proc.lock().expect("Mutex Lock failed");
            if let Some(child) = proc.child.as_mut() {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        proc.child = Some(self.config
                            .get_mut(&name).unwrap()
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
                if let Some(task) = self.config.get_mut(&name) {
                    proc.child = Some(task.command.as_mut().unwrap().spawn().expect("Failed to spawn proc"));
                } else {
                    println!("Unknown Program");
                }
            }
        }
    }

    pub fn build(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let commands: Vec<Mutex<Processus>> = vec!();
        let config = fs::read_to_string(file_path)?;
        let config: HashMap<String, Task> = serde_yaml::from_str(&config)?;

        Ok(Taskmaster {
            procs: commands,
            logger: Logger::new("taskmaster.log"),
            config,
            workQ: Mutex::new(Vec::new())
        })
    }

    pub fn execute(& mut self) -> Result<(), Box<dyn Error>> {
        let mut i = 0;
        for (name, properties) in self.config.iter_mut() {

            Self::build_command(properties)?;

            for id in 0..properties.numprocs {
                self.procs.push(Mutex::new(Processus::build(i + id, name, properties.startretries)));
            }
            i += properties.numprocs;
        }
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
        loop {
            buff.clear();
            io::stdin().read_line(&mut buff).expect("Failed to read");
            let input: Vec<&str> = buff.split_whitespace().collect();
            if let Some(instruct) = input.get(0) {
                match instruct.to_lowercase().as_str() {
                    "exit" => {
                        process::exit(0);
                    }
                    "status" => {
                        let mut queue = self.workQ.lock().expect("Mutex Lock failed");
                        self.status();
                        queue.push(Instruction::Status);
                    }
                    "start" => {
                        if let Some(arg) = input.get(1) {
                            self.start(arg.to_string());
                            let mut queue = self.workQ.lock().expect("Mutex Lock failed");
                            queue.push(Instruction::Start(arg.to_string()));
                        } else {
                            println!("Which program you want to start ? ($ start nginx)");
                        }
                    }
                    "stop" => {
                        if let Some(arg) = input.get(1) {
                            let mut queue = self.workQ.lock().expect("Mutex Lock failed");
                            queue.push(Instruction::Stop(arg.to_string()));
                        } else {
                            println!("Which program you want to stop ? ($ stop nginx)");
                        }
                    }
                    "restart" => {
                        if let Some(arg) = input.get(1) {
                            let mut queue = self.workQ.lock().expect("Mutex Lock failed");
                            queue.push(Instruction::Restart(arg.to_string()));
                        } else {
                            println!("Which program you want to restart ? ($ restart nginx)");
                        }
                    }
                    _ => {
                        println!("Unknown command");
                    }
                }
            }
        }
    }
}