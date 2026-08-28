use std::cell::RefCell;
use std::rc::Rc;

/// Signal ID for tracking dependencies
pub type SignalId = u64;

/// Signal value holder
#[derive(Debug)]
pub struct Signal<T> {
    id: SignalId,
    value: Rc<RefCell<T>>,
}

impl<T: Clone + 'static> Signal<T> {
    /// Create a new signal with initial value
    pub fn new(value: T) -> Self {
        Self {
            id: next_signal_id(),
            value: Rc::new(RefCell::new(value)),
        }
    }

    /// Read the current value
    pub fn get(&self) -> T {
        self.value.borrow().clone()
    }

    /// Read the current value (mutable reference)
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        f(&self.value.borrow())
    }

    /// Get the signal ID
    pub fn id(&self) -> SignalId {
        self.id
    }
}

impl<T: Clone + 'static> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            value: self.value.clone(),
        }
    }
}

/// Signal setter for updating values
#[derive(Debug)]
pub struct SignalSetter<T> {
    id: SignalId,
    value: Rc<RefCell<T>>,
}

impl<T: Clone + 'static> SignalSetter<T> {
    /// Set a new value
    pub fn set(&self, value: T) {
        *self.value.borrow_mut() = value;
        notify_subscribers(self.id);
    }

    /// Update value using a closure
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        f(&mut self.value.borrow_mut());
        notify_subscribers(self.id);
    }

    /// Get the signal ID
    pub fn id(&self) -> SignalId {
        self.id
    }
}

impl<T: Clone + 'static> Clone for SignalSetter<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            value: self.value.clone(),
        }
    }
}

/// Create a signal and return (getter, setter)
pub fn create_signal<T: Clone + 'static>(initial: T) -> (Signal<T>, SignalSetter<T>) {
    let signal = Signal::new(initial);
    let setter = SignalSetter {
        id: signal.id,
        value: signal.value.clone(),
    };
    (signal, setter)
}

/// Memo for derived values (cached computation)
#[derive(Debug, Clone)]
pub struct Memo<T: Clone + 'static> {
    signal: Signal<T>,
}

impl<T: Clone + 'static> Memo<T> {
    /// Read the memoized value
    pub fn get(&self) -> T {
        self.signal.get()
    }
}

/// Create a memoized derived value
pub fn create_memo<T: Clone + 'static, F: Fn() -> T>(compute: F) -> Memo<T> {
    Memo {
        signal: Signal::new(compute()),
    }
}

static mut SIGNAL_COUNTER: u64 = 0;

fn next_signal_id() -> u64 {
    unsafe {
        SIGNAL_COUNTER += 1;
        SIGNAL_COUNTER
    }
}

fn notify_subscribers(_id: SignalId) {
    // TODO: Effect scheduling
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_basic() {
        let (count, set_count) = create_signal(0);
        assert_eq!(count.get(), 0);
        set_count.set(5);
        assert_eq!(count.get(), 5);
    }

    #[test]
    fn test_signal_update() {
        let (count, set_count) = create_signal(10);
        set_count.update(|c| *c += 5);
        assert_eq!(count.get(), 15);
    }
}
