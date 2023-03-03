use std::error::Error;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use std::fmt;

use crate::signal::Signal;
use crate::sys::Libc;

#[derive(Debug, PartialEq)]
pub enum Status {
    Starting,
    Stoping,
    Active,
    Inactive,
    Remove,
}

impl fmt::Display for Status {
    //todo understand formating with fmt::Display
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "      {:?}      ", self)
    }
}

pub struct Processus {
    pub id: usize,
    pub name: String,
    pub child: Option<Child>,
    pub retries: usize,
    pub timer: Instant,
    pub status: Status,
}

impl Processus {
    pub fn new(id: usize, name: &str, retries: usize) -> Self {
        Self {
            id,
            name: name.to_owned(),
            child: None,
            retries,
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

    pub fn stop_child(&mut self, signal: Signal) -> Result<(), Box<dyn Error>> {
        Libc::kill(&mut self.child, signal).map_err(|err| format!("Libc::kill function failed: {err}"))?;
        self.start_timer();
        self.status = Status::Stoping;
        Ok(())
    }

    pub fn start_child(&mut self, command: &mut Command, start_retries: usize, mask: u32) -> Result<(), Box<dyn Error>> {
        self.child = Some(Libc::umask(command, mask).map_err(|err| format!("Libc::umask function failed: {err}"))?);
        self.start_timer();
        self.status = Status::Starting;
        Ok(())
    }

    pub fn reset_child(&mut self) {
        self.child = None;
        self.status = Status::Inactive;
    }
}