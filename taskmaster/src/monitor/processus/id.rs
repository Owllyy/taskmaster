use std::{fmt, sync::atomic::{AtomicUsize, Ordering}};

static ID: AtomicUsize = AtomicUsize::new(0);

#[derive(PartialEq, Clone, Copy)]
pub struct Id {
    value: usize,
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl Default for Id {
    fn default() -> Self {
        let id_value;
        id_value = ID.load(Ordering::Relaxed);
        ID.store(id_value, Ordering::Relaxed);
        Self {
            value: id_value,
        }
    }
}