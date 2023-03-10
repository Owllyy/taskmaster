use std::{str::FromStr, error::Error};

use super::processus::Status;
use super::processus::id::Id;

#[derive(Debug)]
pub enum Instruction {
    Status,
    Start(Vec<String>),
    Stop(Vec<String>),
    Restart(Vec<String>),
    Reload,
    RemoveProcessus(Id),
    StartProcessus(Id),
    ResetProcessus(Id),
    RetryStartProcessus(Id),
    SetStatus(Id, Status),
    KillProcessus(Id),
    Exit,
}

impl FromStr for Instruction {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split_whitespace();
        let command_name = parts.next().ok_or("Empty instruction")?;
        match command_name {
            "exit" | "Exit" => Ok(Instruction::Exit),
            "status" | "Status" => Ok(Instruction::Status),
            "start" | "Start" => Ok(Instruction::Start(parts.map(|s| s.to_string()).collect())),
            "stop" | "Stop" => Ok(Instruction::Stop(parts.map(|s| s.to_string()).collect())),
            "restart" | "Restart" => Ok(Instruction::Restart(parts.map(|s| s.to_string()).collect())),
            "reload" | "Reload" => Ok(Instruction::Reload),
            _ => Err("Unknown command".into()),
        }
    }
}

