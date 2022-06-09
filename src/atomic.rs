use std::sync::atomic::{Ordering, AtomicU32};

pub struct NewStringGenerator {
    i: AtomicU32,
}

impl NewStringGenerator {
    pub fn new_string(&mut self) -> String {
        format!("_{}", self.i.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for NewStringGenerator {
    fn default() -> Self {
        Self {i: AtomicU32::new(0)}
    }
}
