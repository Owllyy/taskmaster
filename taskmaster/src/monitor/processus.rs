use std::error::Error;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use std::fmt;

use crate::signal::Signal;
use crate::sys::Libc;

use self::id::Id;

use super::program::Program;

pub mod id;

#[derive(Debug, PartialEq)]
pub enum Status {
    Starting,
    Stoping,
    Active,
    Inactive,
    Reloading,
}

impl fmt::Display for Status {
    //todo understand formating with fmt::Display
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Status::Starting => write!(f, "{:^20}", "Starting".to_string()),
            Status::Stoping => write!(f, "{:^20}", "Stoping"),
            Status::Active => write!(f, "{:^20}", "Active"),
            Status::Inactive => write!(f, "{:^20}", "Inactive"),
            Status::Reloading => write!(f, "{:^20}", "Reloading"),
        }
    }
}

#[derive(Debug)]
pub struct Processus {
    pub id: Id,
    pub name: String,
    pub child: Option<Child>,
    pub retries: usize,
    pub timer: Instant,
    pub status: Status,
}

impl Processus {
    pub fn new(name: &str, program: &Program) -> Self {
        Self {
            id: Default::default(),
            name: name.to_owned(),
            child: None,
            retries: program.config.startretries,
            timer: Instant::now(),
            status: Status::Inactive,
        }
    }

    pub fn start_timer(&mut self) {
        self.timer = Instant::now();
    }

    pub fn is_timeout(&self, duration: usize) -> bool {
        Duration::from_secs(duration as u64) < self.timer.elapsed()
    }

    pub fn stop_child(&mut self, signal: Signal, start_retries: usize) -> Result<(), Box<dyn Error>> {
        Libc::kill(&mut self.child, signal).map_err(|err| format!("Libc::kill function failed: {err}"))?;
        self.start_timer();
        if self.status != Status::Reloading {
            self.status = Status::Stoping;
        }
        self.retries = start_retries;
        Ok(())
    }

    pub fn start_child(&mut self, command: &mut Command, start_retries: usize, mask: u32, restart: bool) -> Result<bool, Box<dyn Error>> {
        if restart && self.retries == 0 {
            self.status = Status::Inactive;
            self.retries = start_retries;
            self.child = None;
            Ok(true)
        } else {
            self.status = Status::Starting;
            if restart {
                self.retries -= 1;
            }
            self.child = Some(Libc::umask(command, mask).map_err(|err| {
                self.reset_child(start_retries);
                format!("Child {} spawn failed: {err}", self.name)})?);
            self.start_timer();
            Ok(false)
        }
    }

    pub fn reset_child(&mut self, start_retries: usize) {
        self.child = None;
        self.status = Status::Inactive;
        self.retries = start_retries;
    }
}