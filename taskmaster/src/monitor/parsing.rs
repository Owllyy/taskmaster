use std::{collections::HashMap, fs};
use std::error::Error;
use std::path::PathBuf;
use serde::{Deserialize, Deserializer};

use crate::monitor::program::Program;
use crate::signal::Signal;

#[derive(Deserialize, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub cmd: String,
    pub numprocs: usize,
    #[serde(deserialize_with = "umask_deserialize")]
    pub umask: u32,
    #[serde(deserialize_with = "workingdir_deserialize")]
    pub workingdir: PathBuf,
    pub autostart: bool,
    #[serde(deserialize_with = "autorestart_deserialize")]
    pub autorestart: String,
    pub exitcodes: Vec<i32>,
    pub startretries: usize,
    pub starttime: usize,
    pub stopsignal: Signal,
    pub stoptime: usize,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
    pub env: HashMap<String, String>,
}

fn umask_deserialize<'de, D>(deserializer: D) -> Result<u32, D::Error> where D: Deserializer<'de> {
    let buf = String::deserialize(deserializer)?;
    
    u32::from_str_radix(&buf.parse::<String>().map_err(serde::de::Error::custom)?, 8).map_err(serde::de::Error::custom)
}

fn workingdir_deserialize<'de, D>(deserializer: D) -> Result<PathBuf, D::Error> where D: Deserializer<'de> {
    let buf = PathBuf::deserialize(deserializer)?;

    if !buf.is_dir() {
        Err(format!("Invalid working directory: {}", buf.to_str().unwrap())).map_err(serde::de::Error::custom)
    } else {
        Ok(buf)
    }
}

fn autorestart_deserialize<'de, D>(deserializer: D) -> Result<String, D::Error> where D: Deserializer<'de> {
    let buf = String::deserialize(deserializer)?;

    match buf.as_str() {
        "always" | "Always" | "never" | "Never" | "unexpected" | "Unexpected" => Ok(buf),
        _ => Err("Invalid autostart parameter: always, never, unexpected").map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize)]
pub struct Parsing {
    #[serde(flatten)]
    pub tasks: HashMap<String, Config>,
}

impl Parsing {
    pub fn parse(file_path: &PathBuf) -> Result<HashMap<String, Program>, Box<dyn Error>> {
        let mut programs: HashMap<String, Program> = HashMap::new();
        let file_content = fs::read_to_string(file_path)?;
        let mut parsed: Parsing = serde_yaml::from_str(&file_content)?;

        for (name, config) in parsed.tasks.drain() {
            programs.insert(name, Program::new(config, None, true));
        }
        Ok(programs)
    }
}