use std::sync::{Mutex, Arc};

pub enum Instruction {
    Status,
    Start(Vec<String>),
    Stop(Vec<String>),
    Restart(Vec<String>),
    Reload(String),
}

pub struct WorkQueue {
    queue: Arc<Mutex<Vec<Instruction>>>,
}

impl WorkQueue {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn clone(&self) -> WorkQueue {
        WorkQueue {
            queue: self.queue.clone(),
        }
    }

    pub fn pop(&self) -> Option<Instruction> {
        let mut queue = self.queue.lock().expect("Failed to lock the work_queue");
        queue.pop()
    }

    pub fn push(&self, instruction: Instruction) {
        let mut queue = self.queue.lock().expect("Failed to lock the work_queue");
        queue.push(instruction);
    }
}