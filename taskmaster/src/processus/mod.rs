use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::fmt;

use crate::signal::Signal;
use crate::{mode_t, Logger, umask, kill};

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
    pub fn build(id: usize, name: &str, retries: usize) -> Self {
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

    pub fn stop_child(&mut self, signal: &String, logger: &Arc<Mutex<Logger>>) {
        let sid = Signal::parse(&signal).unwrap_or(Signal::SIGTERM);
        unsafe {
            if kill(self.child.as_mut().unwrap().id() as i32, sid as i32) < 0 {
                panic!("Failed to kill process");
            }
        }
        self.set_timer();
        self.status = Status::Stoping;
        logger.lock().expect("Mutex lock failed").log(&format!("    stoped process - {} {}", self.id, self.name));
    }

    pub fn start_child(&mut self, command: &mut Command, start_retries: usize, mask: mode_t, logger: &Arc<Mutex<Logger>>) {
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
        logger.lock().expect("Mutex lock failed").log(&format!("    started process - {} {}", self.id, self.name));
    }

    pub fn reset_child(&mut self) {
        self.child = None;
        self.status = Status::Inactive;
    }
}