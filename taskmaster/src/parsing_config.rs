use std::fs;
use std::process::Command;
use serde::Deserialize;
use std::error::Error;
use std::collections::HashMap;


type ModeT = u32;

extern "C" {
    fn umask(mask: ModeT) -> ModeT;
}

#[derive(Deserialize, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields, default)]
struct Job {
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
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Config {
    #[serde(flatten)]
    jobs: HashMap<String, Job>,
}

impl Config {
    pub fn build(file_path: &str) -> Result<Config, Box<dyn Error>> {
        let config = fs::read_to_string(file_path)?;
        let config: Config = serde_yaml::from_str(&config)?;
        let cmd: Vec<&str> = config.jobs.get("ls").unwrap().cmd.split_ascii_whitespace().collect();
        let proc = Command::new(cmd[0])
            .args(&cmd[1..])
            .spawn()
            .expect("Failed to execute the first command");
        Ok(config)
    }
}