use std::process::{Child, Command};
use std::time::{Duration, Instant};
use std::fmt;

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

    pub fn stop_child(&mut self, signal: &String) {
        match Command::new("kill")
        // TODO: replace `TERM` to signal you want.
        .args(["-s", signal, &self.child.as_ref().unwrap().id().to_string()])
        .spawn() {
            Ok(_) => {},
            Err(e) => panic!("Fail to send the stop signal : {e}"),
        }
        self.set_timer();
        self.status = Status::Stoping;
    }

    pub fn start_child(&mut self, command: &mut Command, startRetries: usize) {
        self.child = Some(command.spawn().expect("Failed to spawn self"));
        self.set_timer();
        self.status = Status::Starting;
        self.retries = startRetries;
    }

    pub fn reset_child(&mut self) {
        self.child = None;
        self.status = Status::Inactive;
    }
}