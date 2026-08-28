use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Context provider for sharing state
pub struct Context {
    values: HashMap<TypeId, Box<dyn Any>>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn provide<T: 'static>(&mut self, value: T) {
        self.values.insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref())
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
