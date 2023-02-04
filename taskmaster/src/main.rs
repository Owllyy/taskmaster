extern crate yaml_rust;

use yaml_rust::{YamlLoader, YamlEmitter, Yaml};
use std::fs;
use std::error::Error;
use std::collections::HashMap;
use std::process;

#[derive(Debug)]
struct Job {
    name: String,
    cmd: String,
    numprocs: usize,
    umask: usize,
    workingdir: String,
    autostart: bool,
    autorestart: String,
    // exitcodes: Vec<u8>,
    startretries: usize,
    starttime: usize,
    stopsignal: String,
    stoptime: usize,
    // stdout: std::io::Stdout,
    // stderr: std::io::Stderr,
    // env: HashMap<String, String>,
}

impl Job {
    fn build(program_name: &Yaml, value: &Yaml) -> Result<Job, Box<dyn Error>> {
        let mut job = Job {
            name: program_name.as_str().unwrap().to_owned(),
            cmd: String::new(),
            numprocs: 0,
            umask: 0,
            workingdir: String::new(),
            autostart: false,
            autorestart: String::new(),
            startretries: 0,
            starttime: 0,
            stopsignal: String::new(),
            stoptime: 0,
        };
        println!("{}", job.name);
        
        for (k, v) in value.as_hash().expect("fatal").iter() {
            match k.as_str().unwrap_or("") {
                "cmd" => job.cmd = v.as_str().unwrap().to_owned(),
                "numprocs" => job.numprocs = usize::try_from(v.as_i64().unwrap()).unwrap_or(1),
                "umask" => job.umask = v.as_i64().unwrap() as usize,
                "workindir" => job.workingdir = v.as_str().unwrap().to_owned(),
                "autostart" => job.autostart = v.as_bool().unwrap(),
                "autorestart" => job.autorestart = v.as_str().unwrap().to_owned(),
                "startretries" => job.startretries = v.as_i64().unwrap() as usize,
                "starttime" => job.starttime = v.as_i64().unwrap() as usize,
                "stopsignal" => job.stopsignal = v.as_str().unwrap().to_owned(),
                "stoptime" => job.stoptime = v.as_i64().unwrap() as usize,
                _ => {/* Err("Unknown field") */},
            }
        }
        println!("{:?}", job);
        Ok(job)
    }

    // fn get_fs(path: &str) -> Result<fs::File, Box<dyn Error>> {
    //     fs
    // }
}
struct Config {
    jobs: Vec<Job>,
}

impl Config {
    fn build(file_path: &str) -> Result<(), Box<dyn Error>> {
        let mut config = Config {
            jobs: Vec::new(),
        };
        let docs = YamlLoader::load_from_str(fs::read_to_string(file_path)?.as_str())?;
        let doc = &docs[0];

        if doc["programs"].is_badvalue() {
            Err("missing \"programs\" field")?;
        }

        //TODO dorian
        for (name, value) in doc["programs"].as_hash().expect("Fatal").iter() {
            config.jobs.push(Job::build(name, value)?);
        }

        Ok(())
    }
}

fn main() {

    Config::build("template.conf").unwrap_or_else(|err| {
        eprintln!("Problem parsing configuration file: {err}");
        process::exit(1);
    });
}
