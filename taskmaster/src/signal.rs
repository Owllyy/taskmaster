use std::error::Error;

#[allow(non_camel_case_types)]
pub type c_int = i32;
#[allow(non_camel_case_types)]
pub type sighandler_t = usize;

extern "C" {
    pub fn signal(
        signum: c_int, 
        handler: sighandler_t,
    ) -> sighandler_t;
}

pub const SIG_IGN: usize = 1;

#[repr(i32)]
pub enum Signal {
    SIGHUP = 1,
    SIGINT = 2,
    SIGQUIT = 3,
    SIGILL = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGEMT = 7,
    SIGFPE = 8,
    SIGKILL = 9,
    SIGBUS = 10,
    SIGSEGV = 11,
    SIGSYS = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGURG = 16,
    SIGSTOP = 17,
    SIGTSTP = 18,
    SIGCONT = 19,
    SIGCHLD = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
    SIGIO = 23,
    SIGXCPU = 24,
    SIGXFSZ = 25,
    SIGVTALRM = 26,
    SIGPROF = 27,
    SIGWINCH = 28,
    SIGINFO = 29,
    SIGUSR1 = 30,
    SIGUSR2 = 31,
}

impl Signal {
    pub fn parse(value: &str) -> Result<Signal, Box<dyn Error>> {
        match value {
            "SIGHUP" => Ok(Signal::SIGHUP),
            "SIGINT" => Ok(Signal::SIGINT),
            "SIGQUIT" => Ok(Signal::SIGQUIT),
            "SIGILL" => Ok(Signal::SIGILL),
            "SIGTRAP" => Ok(Signal::SIGTRAP),
            "SIGABRT" => Ok(Signal::SIGABRT),
            "SIGEMT" => Ok(Signal::SIGEMT),
            "SIGFPE" => Ok(Signal::SIGFPE),
            "SIGKILL" => Ok(Signal::SIGKILL),
            "SIGBUS" => Ok(Signal::SIGBUS),
            "SIGSEGV" => Ok(Signal::SIGSEGV),
            "SIGSYS" => Ok(Signal::SIGSYS),
            "SIGPIPE" => Ok(Signal::SIGPIPE),
            "SIGALRM" => Ok(Signal::SIGALRM),
            "SIGTERM" => Ok(Signal::SIGTERM),
            "SIGURG" => Ok(Signal::SIGURG),
            "SIGSTOP" => Ok(Signal::SIGSTOP),
            "SIGTSTP" => Ok(Signal::SIGTSTP),
            "SIGCONT" => Ok(Signal::SIGCONT),
            "SIGCHLD" => Ok(Signal::SIGCHLD),
            "SIGTTIN" => Ok(Signal::SIGTTIN),
            "SIGTTOU" => Ok(Signal::SIGTTOU),
            "SIGIO" => Ok(Signal::SIGIO),
            "SIGXCPU" => Ok(Signal::SIGXCPU),
            "SIGXFSZ" => Ok(Signal::SIGXFSZ),
            "SIGVTALRM" => Ok(Signal::SIGVTALRM),
            "SIGPROF" => Ok(Signal::SIGPROF),
            "SIGWINCH" => Ok(Signal::SIGWINCH),
            "SIGINFO" => Ok(Signal::SIGINFO),
            "SIGUSR1" => Ok(Signal::SIGUSR1),
            "SIGUSR2" => Ok(Signal::SIGUSR2),
            _ => Err("Unknown signal")?,
        }
    }
}
