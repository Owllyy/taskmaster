use std::fs;
use std::process::{Command, Child};
use serde::Deserialize;
use std::error::Error;
use std::collections::HashMap;

type mode_t = u32;

extern "C" {
    fn umask(mask: mode_t) -> mode_t;
}

#[derive(Debug)]
struct Processus {
    id: usize,
    name: String,
    child: Option<Child>,
}

impl Processus {
    fn build(id: usize, name: &str) -> Self {
        Self {
            id,
            name: name.to_owned(),
            child: None,
        }
    }
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields, default)]
struct Taks {
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

#[derive(Deserialize, Debug)]
pub struct Taskmaster {
    #[serde(skip)]
    procs: Vec<Processus>,
    #[serde(flatten)]
    config: HashMap<String, Taks>,
}

impl Taskmaster {

    pub fn build(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let commands: Vec<Processus> = vec!();
        let config = fs::read_to_string(file_path)?;
        let config: HashMap<String, Taks> = serde_yaml::from_str(&config)?;

        Ok(Taskmaster {
            procs: commands,
            config,
        })
    }

    pub fn execute(& mut self) {
        for (name, properties) in self.config.iter_mut() {
            let args: Vec<&str> = properties.cmd.split_ascii_whitespace().collect();

            //TODO fn build_command() -> Option<Command>
            properties.command = Some(Command::new(properties.cmd.split_once(" ").unwrap().0));
            for id in 0..properties.numprocs {
                self.procs.push(Processus::build(id, name));
            }
        }
    }

    fn build_command(task: Taks) -> Option<Command> {
        let cmd: Vec<&str> = task.cmd.split_whitespace().collect();
        
        task.command = Some(Command::new(cmd.get(0).unwrap()));

        todo!()
    }
}