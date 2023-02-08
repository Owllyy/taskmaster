use std::fs;
use std::fs::File;
use std::process::{Command, Child, Stdio};
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
    retries: usize,
}

impl Processus {
    fn build(id: usize, name: &str, retries: usize) -> Self {
        Self {
            id,
            name: name.to_owned(),
            child: None,
            retries,
        }
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

#[derive(Deserialize, Debug)]
pub struct Taskmaster {
    #[serde(skip)]
    procs: Vec<Processus>,
    #[serde(flatten)]
    config: HashMap<String, Task>,
}

impl Taskmaster {

    pub fn build(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let commands: Vec<Processus> = vec!();
        let config = fs::read_to_string(file_path)?;
        let config: HashMap<String, Task> = serde_yaml::from_str(&config)?;

        Ok(Taskmaster {
            procs: commands,
            config,
        })
    }

    pub fn execute(& mut self) -> Option<Box<dyn Error>> {
        for (name, properties) in self.config.iter_mut() {

            match Self::build_command(properties) {
                Some(error) => return Some(error),
                None => {},
            }

            for id in 0..properties.numprocs {
                self.procs.push(Processus::build(id, name, properties.startretries));
            }
        }
        None
    }

    fn build_command(task: &mut Task) -> Option<Box<dyn Error>> {
        let cmd: Vec<&str> = task.cmd.split_whitespace().collect();
        let output = Self::creat_output(&task).unwrap();
        task.command = Some(Command::new(cmd.get(0).unwrap()));

        task.command.as_mut().unwrap().args(cmd.get(1).iter())
            .envs(task.env.iter())
            .current_dir(&task.workingdir)
            .stdout(output.0)
            .stderr(output.1);
        
        None
    }

    fn creat_output(task: &Task) -> Result<(Stdio, Stdio), Box<dyn Error>> {
        let stdout = Stdio::from(File::open(&task.stdout).unwrap_or(File::create(&task.stdout)?));
        let stderr = Stdio::from(File::open(&task.stderr).unwrap_or(File::create(&task.stderr)?));

        Ok((stdout, stderr))
    }

}