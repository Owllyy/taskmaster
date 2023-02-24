use std::{collections::HashMap, fs};
use std::error::Error;
use serde::Deserialize;

use crate::monitor::program::Program;
use crate::signal::Signal;

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Task {
    pub cmd: String,
    pub numprocs: usize,
    pub umask: String,
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

#[derive(Deserialize)]
pub struct Parsing {
    #[serde(flatten)]
    pub tasks: HashMap<String, Task>,
}

impl Parsing {
    pub fn parse(file_path: &str) -> Result<HashMap<String, Program>, Box<dyn Error>> {
        let mut programs: HashMap<String, Program> = HashMap::new();
        let file_content = fs::read_to_string(file_path)?;
        let mut parsed: Parsing = serde_yaml::from_str(&file_content)?;

        for (name, config) in parsed.tasks.drain() {
            programs.insert(name, Program { config, command: None });
        }
        Ok(programs)
    }
}