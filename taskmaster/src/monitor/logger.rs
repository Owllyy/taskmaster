use std::{io, fs::File, error::Error};

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
    pub fn new(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let output = match File::create(file_path) {
            Ok(file) => Box::new(file),
            Err(_) => Err("Failed to create/open the log file")?,
        };
        Ok(Self {
            output
        })
    }

    pub fn log(&mut self, msg: &str) {
        writeln!(self.output, "{msg}").ok();
    }
}