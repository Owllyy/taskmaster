use std::fmt;

static mut id: usize = 0;

#[derive(PartialEq)]
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
        unsafe {
            id_value = id;
            id += 1;
        }
        Self {
            value: id_value,
        }
    }
}