use std::{io, fs::File};

pub struct Logger {
    output: Box<dyn io::Write + Send + Sync>,
}

impl Default for Logger {
    fn default() -> Self {
        Self {
            output: Box::new(io::stdout()),
        }
    }
}

impl Logger {
    pub fn new(file_path: &str) -> Self {
        let output: Box<dyn io::Write + Send + Sync> = match File::create(file_path) {
            Ok(file) => Box::new(file),
            Err(_) => Box::new(io::stdout()),
        };
        Self {
            output
        }
    }

    pub fn log(&mut self, msg: &str) {
        self.output.write(msg.as_bytes()).expect("Failed to log");
        self.output.write(&[b'\n']).expect("Failed to log");
    }
}