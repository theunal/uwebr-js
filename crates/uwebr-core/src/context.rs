use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;

// ── Instance Context (per-component) ───────────────────────────────────

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

// ── Global Context (shared across components) ──────────────────────────

thread_local! {
    static GLOBAL_CONTEXT: RefCell<HashMap<TypeId, Box<dyn Any>>> = RefCell::new(HashMap::new());
}

/// Provide a context value globally (accessible via use_context in any component)
pub fn provide_context<T: Clone + 'static>(value: T) {
    GLOBAL_CONTEXT.with(|ctx| {
        ctx.borrow_mut().insert(TypeId::of::<T>(), Box::new(value));
    });
}

/// Retrieve a context value from the global context
pub fn use_context<T: Clone + 'static>() -> Option<T> {
    GLOBAL_CONTEXT.with(|ctx| {
        ctx.borrow()
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    })
}

/// Remove a context value from the global context
pub fn remove_context<T: 'static>() {
    GLOBAL_CONTEXT.with(|ctx| {
        ctx.borrow_mut().remove(&TypeId::of::<T>());
    });
}

/// Reset global context (for testing)
pub fn reset_context() {
    GLOBAL_CONTEXT.with(|ctx| {
        ctx.borrow_mut().clear();
    });
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_new() {
        let ctx = Context::new();
        assert_eq!(ctx.get::<i32>(), None);
    }

    #[test]
    fn test_context_provide_get() {
        let mut ctx = Context::new();
        ctx.provide(42i32);
        assert_eq!(ctx.get::<i32>(), Some(&42));
    }

    #[test]
    fn test_context_overwrite() {
        let mut ctx = Context::new();
        ctx.provide(1i32);
        ctx.provide(2i32);
        assert_eq!(ctx.get::<i32>(), Some(&2));
    }

    #[test]
    fn test_context_different_types() {
        let mut ctx = Context::new();
        ctx.provide(42i32);
        ctx.provide("hello".to_string());
        assert_eq!(ctx.get::<i32>(), Some(&42));
        assert_eq!(ctx.get::<String>(), Some(&"hello".to_string()));
    }

    #[test]
    fn test_global_provide_use() {
        reset_context();
        provide_context(99i32);
        assert_eq!(use_context::<i32>(), Some(99));
        reset_context();
    }

    #[test]
    fn test_global_overwrite() {
        reset_context();
        provide_context(1i32);
        provide_context(2i32);
        assert_eq!(use_context::<i32>(), Some(2));
        reset_context();
    }

    #[test]
    fn test_global_different_types() {
        reset_context();
        provide_context(42i32);
        provide_context("world".to_string());
        assert_eq!(use_context::<i32>(), Some(42));
        assert_eq!(use_context::<String>(), Some("world".to_string()));
        reset_context();
    }

    #[test]
    fn test_global_remove() {
        reset_context();
        provide_context(42i32);
        assert_eq!(use_context::<i32>(), Some(42));
        remove_context::<i32>();
        assert_eq!(use_context::<i32>(), None);
        reset_context();
    }

    #[test]
    fn test_global_missing() {
        reset_context();
        assert_eq!(use_context::<i32>(), None);
    }
}
