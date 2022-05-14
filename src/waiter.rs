use parking_lot::{Condvar, Mutex};

#[derive(Debug)]
pub(crate) struct Waiter {
    mutex: Mutex<bool>,
    condvar: Condvar,
}

impl Waiter {
    pub fn new(value: bool) -> Self {
        Self {
            mutex: Mutex::new(value),
            condvar: Condvar::new(),
        }
    }

    pub fn wait(&self) {
        let mut lock = self.mutex.lock();
        while !*lock {
            self.condvar.wait(&mut lock);
        }
    }

    pub fn reset(&self) {
        *self.mutex.lock() = false;
    }

    pub fn set(&self) {
        *self.mutex.lock() = true;
        self.condvar.notify_one();
    }
}
