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
    Start(Vec<String>),
    Stop(Vec<String>),
    Restart(Vec<String>)
}

#[derive(Deserialize)]
pub struct Taskmaster {
    #[serde(skip)]
    procs: Vec<Arc<Mutex<Processus>>>,
    #[serde(skip)]
    logger: Logger,
    #[serde(flatten)]
    config: HashMap<String, Task>,
    #[serde(skip)]
    work_q: Arc<Mutex<Vec<Instruction>>>,
}

impl Taskmaster {

    pub fn executioner(work_q: &Arc<Mutex<Vec<Instruction>>>, procs: &Vec<Arc<Mutex<Processus>>>) {
        let mut arc_proc: Vec<Arc<Mutex<Processus>>> = vec![];
        for proc in procs.iter() {
            arc_proc.push(Arc::clone(&proc));
        }
        let work_q = Arc::clone(&work_q);
        thread::spawn(move || {
            loop {
                let mut queue = work_q.get_mut().expect("Mutex Lock failed");
    
                if let Some(instruction) = queue.pop() {
                    match instruction {
                        Instruction::Status => Taskmaster::status(&arc_proc),
                        Instruction::Start(task) => Taskmaster::start(&arc_proc, task),
                        Instruction::Stop(task) => self.stop(task),
                        Instruction::Restart(task) => self.restart(task),
                    }
                }
            }
        });
    }

    fn status(procs: &Vec<Arc<Mutex<Processus>>>) {
        println!("{:-<55}", "-");
        println!("| {:^5} | {:^20} | {:^20} |", "ID", "NAME", "STATUS");
        println!("{:-<55}", "-");
        for processus in procs.iter() {
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

    fn start(procs: &Vec<Arc<Mutex<Processus>>>, mut config: &HashMap<String, Task>, names: Vec<String>) {
        for name in names {
            for proc in procs.iter() {
                let mut proc = proc.lock().expect("Mutex Lock failed");
                if let Some(child) = proc.child.as_mut() {
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            proc.child = Some(config
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
                    if let Some(task) = config.get_mut(&name) {
                        proc.child = Some(task.command.as_mut().expect("Can't spawn command").spawn().expect("Failed to spawn proc"));
                    } else {
                        println!("Unknown Program");
                    }
                }
            }
        }
    }

    fn stop(&mut self, name: Vec<String>) {
        for name in name {
            for proc in self.procs.iter() {
                let mut proc = proc.lock().expect("Mutex Lock failed");
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
                                .args(["-s", &self.config.get(&name).unwrap().stopsignal, &child.id().to_string()])
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
                    if let Some(task) = self.config.get_mut(&name) {
                        println!("The program {name} is not running");
                    } else {
                        println!("Unknown Program");
                    }
                }
            }
        }
    }

    fn restart(&mut self, name: Vec<String>) {
        self.stop(name.to_owned());
        self.start(name);
    }

    fn start_all(&mut self) {
        let mut all_task = Vec::new();
        for (name, task) in &self.config {
            if (task.autostart) {
                all_task.push(name.to_owned());
            }
        }
        self.start(all_task);
    }

    fn stop_all(&mut self) {
        let mut all_task = Vec::new();
        for (name, _) in &self.config {
            all_task.push(name.to_owned());
        }
        self.stop(all_task);
    }

    pub fn build(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let commands: Vec<Arc<Mutex<Processus>>> = vec!();
        let config = fs::read_to_string(file_path)?;
        let config: HashMap<String, Task> = serde_yaml::from_str(&config)?;

        Ok(Taskmaster {
            procs: commands,
            logger: Logger::new("taskmaster.log"),
            config,
            work_q: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn execute(& mut self) -> Result<(), Box<dyn Error>> {
        let mut i = 0;
        for (name, properties) in self.config.iter_mut() {

            Self::build_command(properties)?;

            for id in 0..properties.numprocs {
                self.procs.push(Arc::new(Mutex::new(Processus::build(i + id, name, properties.startretries))));
            }
            i += properties.numprocs;
        }
        self.start_all();
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
                        let mut queue = self.work_q.lock().expect("Mutex Lock failed");
                        self.status();
                        queue.push(Instruction::Status);
                    }
                    "start" => {
                        if let Some(arg) = input.get(1) {
                            self.start(vec![arg.to_string()]);
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
            }
        }
    }
}