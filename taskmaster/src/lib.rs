use std::{fs, default};
use std::fs::File;
use std::io::{self, Read};
use std::process::{Command, Child, Stdio};
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
    Exit,
    Status,
    Start(String),
    Stop(String),
    Restart(String),
    Unknown,
}

#[derive(Deserialize)]
pub struct Taskmaster {
    #[serde(skip)]
    procs: Mutex<Vec<Processus>>,
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
            let queue = self.workQ.lock().expect("Mutex Lock failed");

            if let Some(instruction) = queue.pop() {
                match instruction {
                    Instruction::Status => self.status(),
                    Instruction::Start(task) => start(task),
                    Instruction::Stop(task) => stop(task),
                    Instruction::Restart(task) => restart(task),
                    _ => {},
                }
            }
        }
        Ok(())
    }

    fn status(&self) {
        let procs = self.procs.lock().expect("Mutex Lock Failed");

        for processus in procs.iter() {
            processus.child
        }
    }

    pub fn build(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let commands: Vec<Processus> = vec!();
        let config = fs::read_to_string(file_path)?;
        let config: HashMap<String, Task> = serde_yaml::from_str(&config)?;

        Ok(Taskmaster {
            procs: Mutex::new(commands),
            logger: Logger::new("taskmaster.log"),
            config,
            workQ: Mutex::new(Vec::new())
        })
    }

    pub fn execute(& mut self) -> Result<(), Box<dyn Error>> {
        for (name, properties) in self.config.iter_mut() {

            Self::build_command(properties)?;
            
            let procs = self.procs.lock().expect("Mutex lock failed");

            for id in 0..properties.numprocs {
                procs.push(Processus::build(id, name, properties.startretries));
            }
        }
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
            io::stdin().read_line(&mut buff).expect("Failed to read");
            let input: Vec<&str> = buff.split_whitespace().collect();
            if let Some(instruct) = input.get(0) {
                match instruct {
                    "exit" => unimplemented!("exit"),
                    "status" => {
                        let queue = self.workQ.lock().expect("Mutex Lock failed");
                        queue.push(Instruction::Status)
                    }
                    "start" => {
                        let queue = self.workQ.lock().expect("Mutex Lock failed");
                        // queue.push(Instruction::Start(instruct.next()))
                    },
                    "stop" => unimplemented!(),
                    "restart" => unimplemented!(),
                    _ => unimplemented!(),
                }
            }
        }
    }
}