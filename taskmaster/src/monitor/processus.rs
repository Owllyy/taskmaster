use std::process::{Child, Command};
use std::time::{Duration, Instant};
use std::fmt;

use crate::signal::Signal;
use crate::{mode_t, umask, kill};

#[derive(Debug)]
pub enum Status {
    Starting,
    Stoping,
    Active,
    Inactive,
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

    pub fn set_timer(&mut self) {
        self.timer = Instant::now();
    }

    pub fn check_timer(&self, duration: usize) -> bool {
        let duration = Duration::from_secs(duration as u64);
        
        if self.timer.elapsed() < duration {
            return false;
        }

        true
    }

    pub fn stop_child(&mut self, signal: &String) {
        let sid = Signal::parse(&signal).unwrap_or(Signal::SIGTERM);
        unsafe {
            if kill(self.child.as_mut().unwrap().id() as i32, sid as i32) < 0 {
                panic!("Failed to stop process");
            }
        }
        self.set_timer();
        self.status = Status::Stoping;
    }

    pub fn start_child(&mut self, command: &mut Command, start_retries: usize, mask: mode_t) {
        let old_mask: mode_t;
        unsafe {
            old_mask = umask(mask);
        }
        self.child = Some(command.spawn().expect("Failed to spawn self"));
        unsafe {
            umask(old_mask);
        }
        self.set_timer();
        self.status = Status::Starting;
        self.retries = start_retries;
    }

    pub fn reset_child(&mut self) {
        self.child = None;
        self.status = Status::Inactive;
    }
}