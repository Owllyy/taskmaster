use std::{error::Error, process::{Child, Command}, sync::atomic::AtomicBool};
use crate::signal::Signal;

extern "C" {
    fn umask(mask: u32) -> u32;
    fn kill(pid: i32, sig: i32) -> i32;
    fn signal(
        signum: i32, 
        handler: usize,
    ) -> usize;
}

const SIG_ERR: usize = 18_446_744_073_709_551_615usize;

pub static RELOAD_INSTRUCTION: AtomicBool = AtomicBool::new(false);

pub struct Libc;

impl Libc {
    pub fn umask(command: &mut Command, mask: u32) -> Result<Child, Box<dyn Error>> {
        let old_mask: u32;
        unsafe {
            old_mask = umask(mask);
        }
        let child = command.spawn()?;
        unsafe {
            umask(old_mask);
        }
        Ok(child)
    }

    pub fn kill(child: &mut Option<Child>, sig: Signal) -> Result<(), Box<dyn Error>> {
        unsafe {
            if kill(child.as_mut().ok_or("child process does not exist")?.id() as i32, sig as i32) != 0 {
                return Err("failed to kill process".into());
            }
        }
        Ok(())
    }

    pub fn signal(sig: Signal, fn_sig_handler: fn(i32)) -> Result<(), Box<dyn Error>> {
        unsafe {
            if signal(sig as i32, fn_sig_handler as usize) == SIG_ERR {
                return Err("the call to signal funtion failed".into());
            }
        }
        Ok(())
    }
}

