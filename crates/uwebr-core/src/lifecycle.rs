use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// Component ID for lifecycle tracking
pub type ComponentId = u64;

thread_local! {
    static LIFECYCLE: RefCell<LifecycleState> = RefCell::new(LifecycleState::new());
    static CURRENT_COMPONENT: std::cell::Cell<Option<ComponentId>> = const { std::cell::Cell::new(None) };
    static NEXT_COMPONENT_ID: Cell<u64> = const { Cell::new(1) };
}

struct LifecycleState {
    mount_callbacks: HashMap<ComponentId, Vec<Box<dyn FnOnce()>>>,
    cleanup_callbacks: HashMap<ComponentId, Vec<Box<dyn FnOnce()>>>,
    mounted: HashMap<ComponentId, bool>,
    hook_states: HashMap<(ComponentId, TypeId), Box<dyn Any>>,
}

impl LifecycleState {
    fn new() -> Self {
        Self {
            mount_callbacks: HashMap::new(),
            cleanup_callbacks: HashMap::new(),
            mounted: HashMap::new(),
            hook_states: HashMap::new(),
        }
    }
}

fn next_component_id() -> ComponentId {
    NEXT_COMPONENT_ID.with(|c| {
        c.set(c.get() + 1);
        c.get()
    })
}

/// Initialize a new component scope, returns its ID
pub fn create_component_scope() -> ComponentId {
    let id = next_component_id();
    LIFECYCLE.with(|lc| {
        let mut lc = lc.borrow_mut();
        lc.mounted.insert(id, false);
    });
    id
}

/// Get the current component ID (if inside a component scope)
pub fn current_component_id() -> Option<ComponentId> {
    CURRENT_COMPONENT.get()
}

/// Run a closure within a component scope
pub fn with_component<F, R>(id: ComponentId, f: F) -> R
where
    F: FnOnce() -> R,
{
    CURRENT_COMPONENT.set(Some(id));
    let result = f();
    CURRENT_COMPONENT.set(None);
    result
}

/// Register a callback to run when the component mounts
pub fn on_mount<F: FnOnce() + 'static>(callback: F) {
    if let Some(component_id) = CURRENT_COMPONENT.get() {
        LIFECYCLE.with(|lc| {
            lc.borrow_mut()
                .mount_callbacks
                .entry(component_id)
                .or_default()
                .push(Box::new(callback));
        });
    }
}

/// Register a callback to run when the component is destroyed
pub fn on_cleanup<F: FnOnce() + 'static>(callback: F) {
    if let Some(component_id) = CURRENT_COMPONENT.get() {
        LIFECYCLE.with(|lc| {
            lc.borrow_mut()
                .cleanup_callbacks
                .entry(component_id)
                .or_default()
                .push(Box::new(callback));
        });
    }
}

/// Trigger mount callbacks for a component (call after first render)
pub fn trigger_mount(component_id: ComponentId) {
    LIFECYCLE.with(|lc| {
        let mut lc = lc.borrow_mut();
        if let Some(false) = lc.mounted.get(&component_id) {
            lc.mounted.insert(component_id, true);
            if let Some(callbacks) = lc.mount_callbacks.remove(&component_id) {
                drop(lc); // release borrow before running callbacks
                for cb in callbacks {
                    cb();
                }
            }
        }
    });
}

/// Trigger cleanup callbacks for a component (call on destroy)
pub fn trigger_cleanup(component_id: ComponentId) {
    LIFECYCLE.with(|lc| {
        let mut lc = lc.borrow_mut();
        lc.mounted.remove(&component_id);
        if let Some(callbacks) = lc.cleanup_callbacks.remove(&component_id) {
            drop(lc);
            for cb in callbacks {
                cb();
            }
        }
    });
}

/// Clean up all component states (for testing)
pub fn reset_lifecycle() {
    LIFECYCLE.with(|lc| {
        lc.borrow_mut().mount_callbacks.clear();
        lc.borrow_mut().cleanup_callbacks.clear();
        lc.borrow_mut().mounted.clear();
        lc.borrow_mut().hook_states.clear();
    });
}

// ── Hook State Storage ─────────────────────────────────────────────────

/// Store a hook state value for the current component
pub fn set_hook_state<T: 'static>(key: TypeId, value: T) {
    if let Some(component_id) = CURRENT_COMPONENT.get() {
        LIFECYCLE.with(|lc| {
            lc.borrow_mut()
                .hook_states
                .insert((component_id, key), Box::new(value));
        });
    }
}

/// Get a hook state value for the current component
pub fn get_hook_state<T: Clone + 'static>(key: TypeId) -> Option<T> {
    let component_id = CURRENT_COMPONENT.get()?;
    LIFECYCLE.with(|lc| {
        lc.borrow()
            .hook_states
            .get(&(component_id, key))
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    })
}

/// Update a hook state value using a closure
pub fn update_hook_state<T: 'static, F: FnOnce(&mut T)>(key: TypeId, f: F) {
    if let Some(component_id) = CURRENT_COMPONENT.get() {
        LIFECYCLE.with(|lc| {
            let mut lc = lc.borrow_mut();
            if let Some(state) = lc.hook_states.get_mut(&(component_id, key)) {
                if let Some(val) = state.downcast_mut::<T>() {
                    f(val);
                }
            }
        });
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    #[test]
    fn test_component_scope() {
        let id = create_component_scope();
        assert!(id > 0);

        let called = Rc::new(Cell::new(false));
        let called_clone = called.clone();

        with_component(id, || {
            on_mount(move || {
                called_clone.set(true);
            });
        });

        trigger_mount(id);
        assert!(called.get());
    }

    #[test]
    fn test_cleanup_callback() {
        let id = create_component_scope();
        let cleaned = Rc::new(Cell::new(false));
        let cleaned_clone = cleaned.clone();

        with_component(id, || {
            on_cleanup(move || {
                cleaned_clone.set(true);
            });
        });

        assert!(!cleaned.get());
        trigger_cleanup(id);
        assert!(cleaned.get());
    }

    #[test]
    fn test_mount_only_runs_once() {
        let id = create_component_scope();
        let count = Rc::new(Cell::new(0));
        let count_clone = count.clone();

        with_component(id, || {
            on_mount(move || {
                count_clone.set(count_clone.get() + 1);
            });
        });

        trigger_mount(id);
        trigger_mount(id); // should not run again
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn test_multiple_mount_callbacks() {
        let id = create_component_scope();
        let a = Rc::new(Cell::new(false));
        let b = Rc::new(Cell::new(false));
        let a_clone = a.clone();
        let b_clone = b.clone();

        with_component(id, || {
            on_mount(move || a_clone.set(true));
            on_mount(move || b_clone.set(true));
        });

        trigger_mount(id);
        assert!(a.get());
        assert!(b.get());
    }

    #[test]
    fn test_hook_state() {
        let id = create_component_scope();
        let key = TypeId::of::<i32>();

        with_component(id, || {
            set_hook_state(key, 42i32);
        });

        CURRENT_COMPONENT.set(Some(id));
        let val = get_hook_state::<i32>(key);
        assert_eq!(val, Some(42));
        CURRENT_COMPONENT.set(None);
    }

    #[test]
    fn test_lifecycle_integration() {
        let id = create_component_scope();
        let order = Rc::new(RefCell::new(Vec::<String>::new()));
        let order_clone = order.clone();

        with_component(id, || {
            let o = order_clone.clone();
            on_mount(move || {
                o.borrow_mut().push("mount".to_string());
            });
        });

        // Simulate component lifecycle
        trigger_mount(id);
        trigger_cleanup(id);

        let o = order.borrow();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0], "mount");
    }
}
