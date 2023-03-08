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
    pub workingdir: String,
    pub autostart: bool,
    pub autorestart: String,
    pub exitcodes: Vec<i32>,
    pub startretries: usize,
    pub starttime: usize,
    pub stopsignal: Signal,
    pub stoptime: usize,
    pub stdout: String,
    pub stderr: String,
    pub env: HashMap<String, String>,
}

fn umask_deserialize<'de, D>(deserializer: D) -> Result<u32, D::Error> where D: Deserializer<'de> {
    let buf = String::deserialize(deserializer)?;

    buf.parse::<u32>().map_err(serde::de::Error::custom)
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