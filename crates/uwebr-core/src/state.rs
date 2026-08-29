//! Keyed reactive state for transpiled `<script>` blocks.
//!
//! A `.uwebr` `<script>` declares its state at the top level:
//!
//! ```js
//! let count = 0;
//! function increment() { count++; }
//! ```
//!
//! Emitting that literally as a module-level `let` is not valid Rust, and a
//! `static mut` would not be reactive. Instead the transpiler rewrites each
//! top-level binding into keyed accessors backed by a [`Signal`], so reads
//! subscribe and writes schedule a repaint.

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::signal::{create_signal, Signal, SignalSetter};

thread_local! {
    /// Script state keyed by the original JS binding name.
    ///
    /// Keyed by name rather than by component instance because component
    /// functions are re-invoked on every render and must observe the same
    /// state each time.
    static SCRIPT_STATE: RefCell<HashMap<String, Box<dyn Any>>> = RefCell::new(HashMap::new());

    /// Runtime interaction state (`:hover`, `:focus`) keyed by the layout tree's
    /// pre-order node index. Separate from `SCRIPT_STATE` because it is driven by
    /// input events (cursor/focus) rather than by reactive signals, and must not
    /// be cleared when script state is reset.
    static ELEMENT_STATE: RefCell<ElementStateStore> = RefCell::new(ElementStateStore::new());
}

/// Interaction state for the element tree, used by stateful pseudo-classes.
///
/// Nodes are identified by their pre-order index in the layout tree, which is
/// stable across re-renders as long as the tree shape does not change.
#[derive(Debug, Default)]
pub struct ElementStateStore {
    /// Node indices currently under the cursor.
    hovered: HashSet<usize>,
    /// The focused node, if any.
    focused: Option<usize>,
}

impl ElementStateStore {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Mark (or unmark) a node as hovered. Called as the cursor moves.
pub fn set_hovered(node_id: usize, hovered: bool) {
    ELEMENT_STATE.with(|s| {
        let mut store = s.borrow_mut();
        if hovered {
            store.hovered.insert(node_id);
        } else {
            store.hovered.remove(&node_id);
        }
    });
}

/// Set (or clear) the focused node. Called on focus/blur.
pub fn set_focused(node_id: Option<usize>) {
    ELEMENT_STATE.with(|s| s.borrow_mut().focused = node_id);
}

/// Whether a node is currently hovered.
pub fn is_hovered(node_id: usize) -> bool {
    ELEMENT_STATE.with(|s| s.borrow().hovered.contains(&node_id))
}

/// Whether a node is currently focused.
pub fn is_focused(node_id: usize) -> bool {
    ELEMENT_STATE.with(|s| s.borrow().focused == Some(node_id))
}

/// Whether any node is focused (used by `:focus-within` on ancestors).
pub fn any_focused() -> bool {
    ELEMENT_STATE.with(|s| s.borrow().focused.is_some())
}

/// Drop all hover state (called at the start of a hover recomputation). Focus
/// is intentionally preserved: it survives cursor movement.
pub fn clear_hover() {
    ELEMENT_STATE.with(|s| s.borrow_mut().hovered.clear());
}

/// Drop all interaction state (used by tests).
pub fn clear_element_state() {
    ELEMENT_STATE.with(|s| {
        let mut store = s.borrow_mut();
        store.hovered.clear();
        store.focused = None;
    });
}

/// Get (or lazily create) the signal pair behind a script binding.
///
/// `key` is generic over `AsRef<str>` because the JS codegen emits string
/// literals as `"count".to_string()`.
pub fn use_state<T: Clone + 'static>(
    key: impl AsRef<str>,
    initial: T,
) -> (Signal<T>, SignalSetter<T>) {
    let key = key.as_ref();
    let existing = SCRIPT_STATE.with(|s| {
        s.borrow()
            .get(key)
            .and_then(|v| v.downcast_ref::<Signal<T>>())
            .cloned()
    });

    if let Some(signal) = existing {
        let setter = signal.setter();
        return (signal, setter);
    }

    let (signal, setter) = create_signal(initial);
    SCRIPT_STATE.with(|s| {
        s.borrow_mut()
            .insert(key.to_string(), Box::new(signal.clone()));
    });
    (signal, setter)
}

/// Read a script binding, creating it from `initial` on first access.
pub fn get<T: Clone + 'static>(key: impl AsRef<str>, initial: T) -> T {
    let (signal, _) = use_state(key, initial);
    signal.get()
}

/// Write a script binding. Marks the UI dirty via the signal setter.
///
/// The value doubles as the lazy initialiser, so a handler that fires before the
/// component has rendered still works.
pub fn set<T: Clone + 'static>(key: impl AsRef<str>, value: T) {
    let (_, setter) = use_state(key, value.clone());
    setter.set(value);
}

/// Whether a binding has been created.
pub fn contains(key: impl AsRef<str>) -> bool {
    SCRIPT_STATE.with(|s| s.borrow().contains_key(key.as_ref()))
}

/// Drop all script state (used by tests and on hot reload).
pub fn clear() {
    SCRIPT_STATE.with(|s| s.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::{is_render_dirty, take_render_dirty};

    #[test]
    fn test_get_creates_with_initial() {
        clear();
        assert_eq!(get("a", 5i64), 5);
        assert!(contains("a"));
    }

    #[test]
    fn test_set_then_get_roundtrip() {
        clear();
        let _ = get("count", 0i64);
        set("count", 7i64);
        assert_eq!(get("count", 0i64), 7);
    }

    #[test]
    fn test_initial_ignored_after_first_creation() {
        // The component re-renders and passes the literal initial value again;
        // it must not clobber the current state.
        clear();
        let _ = get("c", 0i64);
        set("c", 42i64);
        assert_eq!(get("c", 0i64), 42, "initial must not reset live state");
    }

    #[test]
    fn test_set_marks_render_dirty() {
        clear();
        let _ = get("d", 0i64);
        take_render_dirty();
        set("d", 1i64);
        assert!(is_render_dirty(), "state writes must trigger a repaint");
    }

    #[test]
    fn test_use_state_returns_same_signal() {
        clear();
        let (s1, set1) = use_state("e", 1i64);
        set1.set(9);
        let (s2, _) = use_state("e", 1i64);
        assert_eq!(s2.get(), 9);
        assert_eq!(s1.id(), s2.id(), "same underlying signal");
    }

    #[test]
    fn test_distinct_keys_are_independent() {
        clear();
        set("x", 1i64);
        set("y", 2i64);
        assert_eq!(get("x", 0i64), 1);
        assert_eq!(get("y", 0i64), 2);
    }

    #[test]
    fn test_string_state() {
        clear();
        assert_eq!(get("name", "abc".to_string()), "abc");
        set("name", "def".to_string());
        assert_eq!(get("name", "abc".to_string()), "def");
    }

    #[test]
    fn test_clear_removes_state() {
        clear();
        set("z", 3i64);
        clear();
        assert!(!contains("z"));
        assert_eq!(get("z", 0i64), 0);
    }

    #[test]
    fn test_set_before_first_read_works() {
        // Event handlers may fire before the component reads the binding.
        clear();
        set("early", 11i64);
        assert_eq!(get("early", 0i64), 11);
    }

    #[test]
    fn test_owned_string_key_accepted() {
        // The JS codegen emits literals as `"count".to_string()`.
        clear();
        let key = String::from("count");
        set(&key, 3i64);
        assert_eq!(get(&key, 0i64), 3);
        assert!(contains("count"));
    }

    // ── Element interaction state (FAZ 14) ──────────────────────

    #[test]
    fn test_element_state_hover() {
        clear_element_state();
        assert!(!is_hovered(3));
        set_hovered(3, true);
        assert!(is_hovered(3));
        set_hovered(3, false);
        assert!(!is_hovered(3));
    }

    #[test]
    fn test_element_state_focus() {
        clear_element_state();
        assert!(!is_focused(5));
        assert!(!any_focused());
        set_focused(Some(5));
        assert!(is_focused(5));
        assert!(any_focused());
        assert!(!is_focused(6));
        set_focused(None);
        assert!(!is_focused(5));
        assert!(!any_focused());
    }

    #[test]
    fn test_clear_hover_keeps_focus() {
        clear_element_state();
        set_hovered(1, true);
        set_focused(Some(2));
        clear_hover();
        assert!(!is_hovered(1), "hover cleared");
        assert!(is_focused(2), "focus preserved across hover clear");
    }
}
