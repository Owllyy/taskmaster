use std::{fmt, sync::atomic::{AtomicUsize, Ordering}};

static ID: AtomicUsize = AtomicUsize::new(0);

#[derive(PartialEq, Clone, Copy, Debug)]
pub struct Id {
    value: usize,
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:^5}", self.value)
    }
}

impl Default for Id {
    fn default() -> Self {
        let id_value;
        id_value = ID.load(Ordering::SeqCst);
        ID.store(id_value + 1, Ordering::SeqCst);
        Self {
            value: id_value,
        }
    }
}