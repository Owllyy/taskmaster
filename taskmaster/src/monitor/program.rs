use std::{process::{Command, Stdio}, error::Error, fs::File};
use super::parsing::Task;

pub struct Program {
    pub config: Task,
    pub command: Option<Command>,
}

impl Program {
    pub fn build_command(&mut self) -> Result<(), Box<dyn Error>> {
        let cmd: Vec<&str> = self.config.cmd.split_whitespace().collect();
        let output = self.fd_setup()?;
        self.command = Some(Command::new(cmd.get(0).unwrap()));

        self.command.as_mut().unwrap().args(cmd.get(1).iter())
            .envs(self.config.env.iter())
            .current_dir(&self.config.workingdir)
            .stdout(output.0)
            .stderr(output.1);
        
        Ok(())
    }

    fn fd_setup(&self) -> Result<(Stdio, Stdio), Box<dyn Error>> {
        let stdout = Stdio::from(File::open(&self.config.stdout).unwrap_or(File::create(&self.config.stdout)?));
        let stderr = Stdio::from(File::open(&self.config.stderr).unwrap_or(File::create(&self.config.stderr)?));

        Ok((stdout, stderr))
    }
}