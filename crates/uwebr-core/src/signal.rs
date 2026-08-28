use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

/// Signal ID for tracking dependencies
pub type SignalId = u64;
pub type EffectId = u64;

// ── Global Reactive Runtime ────────────────────────────────────────────

thread_local! {
    static CURRENT_EFFECT: Cell<Option<EffectId>> = Cell::new(None);
    static SIGNAL_COUNTER: Cell<u64> = const { Cell::new(0) };
    /// Effect closures: separate from tracking state to avoid re-entrant borrow
    static EFFECTS: RefCell<HashMap<EffectId, EffectState>> = RefCell::new(HashMap::new());
    /// Which effects subscribe to which signals
    static SIGNAL_SUBS: RefCell<HashMap<SignalId, HashSet<EffectId>>> = RefCell::new(HashMap::new());
    /// Which signals each effect depends on (for cleanup)
    static EFFECT_DEPS: RefCell<HashMap<EffectId, HashSet<SignalId>>> = RefCell::new(HashMap::new());
    /// Dirty effects pending flush
    static DIRTY: RefCell<HashSet<EffectId>> = RefCell::new(HashSet::new());
    static NEXT_EFFECT_ID: Cell<u64> = const { Cell::new(1) };
}

struct EffectState {
    f: Box<dyn FnMut()>,
    #[allow(dead_code)]
    name: String,
}

fn next_signal_id() -> SignalId {
    SIGNAL_COUNTER.with(|c| {
        c.set(c.get() + 1);
        c.get()
    })
}

fn next_effect_id() -> EffectId {
    NEXT_EFFECT_ID.with(|c| {
        c.set(c.get() + 1);
        c.get()
    })
}

/// Track that the current effect reads a signal
fn track_read(signal_id: SignalId) {
    if let Some(effect_id) = CURRENT_EFFECT.get() {
        SIGNAL_SUBS.with(|s| {
            s.borrow_mut()
                .entry(signal_id)
                .or_default()
                .insert(effect_id);
        });
        EFFECT_DEPS.with(|d| {
            d.borrow_mut()
                .entry(effect_id)
                .or_default()
                .insert(signal_id);
        });
    }
}

/// Mark signal as changed, scheduling subscribed effects
fn mark_dirty(signal_id: SignalId) {
    let to_mark: Vec<EffectId> = SIGNAL_SUBS.with(|s| {
        s.borrow()
            .get(&signal_id)
            .map(|subs| subs.iter().copied().collect())
            .unwrap_or_default()
    });
    DIRTY.with(|d| {
        let mut d = d.borrow_mut();
        for id in to_mark {
            d.insert(id);
        }
    });
}

/// Run all dirty effects. Safe against re-entrant borrows
/// because track_read borrows SIGNAL_SUBS/EFFECT_DEPS, not EFFECTS.
pub fn flush_effects() {
    loop {
        let dirty: Vec<EffectId> = DIRTY.with(|d| d.borrow_mut().drain().collect());
        if dirty.is_empty() {
            break;
        }

        for effect_id in dirty {
            // Unsubscribe old deps
            EFFECT_DEPS.with(|d| {
                let mut d = d.borrow_mut();
                if let Some(deps) = d.remove(&effect_id) {
                    for sid in deps {
                        SIGNAL_SUBS.with(|s| {
                            if let Some(subs) = s.borrow_mut().get_mut(&sid) {
                                subs.remove(&effect_id);
                            }
                        });
                    }
                }
            });

            run_effect(effect_id);
        }
    }
}

// ── Signal ─────────────────────────────────────────────────────────────

/// Signal value holder
#[derive(Debug)]
pub struct Signal<T> {
    id: SignalId,
    value: Rc<RefCell<T>>,
}

use std::rc::Rc;

impl<T: Clone + 'static> Signal<T> {
    pub fn new(value: T) -> Self {
        Self {
            id: next_signal_id(),
            value: Rc::new(RefCell::new(value)),
        }
    }

    /// Read the current value (subscribes to current effect if any)
    pub fn get(&self) -> T {
        track_read(self.id);
        self.value.borrow().clone()
    }

    /// Read the current value immutably (subscribes to current effect)
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        track_read(self.id);
        f(&self.value.borrow())
    }

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
    pub fn set(&self, value: T) {
        *self.value.borrow_mut() = value;
        mark_dirty(self.id);
        flush_effects();
    }

    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        f(&mut self.value.borrow_mut());
        mark_dirty(self.id);
        flush_effects();
    }

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

// ── Effect ─────────────────────────────────────────────────────────────

/// Create a reactive effect that runs when its dependencies change
pub fn create_effect<F: FnMut() + 'static>(name: &str, f: F) {
    let effect_id = next_effect_id();
    EFFECTS.with(|e| {
        e.borrow_mut().insert(
            effect_id,
            EffectState {
                f: Box::new(f),
                name: name.to_string(),
            },
        );
    });

    // Run immediately to collect dependencies
    run_effect(effect_id);
}

/// Run a single effect
fn run_effect(effect_id: EffectId) {
    // track_read only borrows SIGNAL_SUBS and EFFECT_DEPS, not EFFECTS,
    // so holding the EFFECTS borrow while calling (state.f)() is safe.
    CURRENT_EFFECT.set(Some(effect_id));
    EFFECTS.with(|e| {
        let mut e = e.borrow_mut();
        if let Some(state) = e.get_mut(&effect_id) {
            (state.f)();
        }
    });
    CURRENT_EFFECT.set(None);
}

// ── Memo ───────────────────────────────────────────────────────────────

/// Memo for derived values (cached, re-evaluates when dependencies change)
pub struct Memo<T: Clone + 'static> {
    signal: Signal<T>,
    #[allow(dead_code)]
    effect_id: EffectId,
}

impl<T: Clone + 'static> Memo<T> {
    pub fn get(&self) -> T {
        track_read(self.signal.id);
        self.signal.get()
    }
}

impl<T: Clone + 'static> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self {
            signal: self.signal.clone(),
            effect_id: self.effect_id,
        }
    }
}

/// Create a memoized derived value that re-evaluates when dependencies change
pub fn create_memo<T: Clone + 'static + PartialEq, F: FnMut() -> T + 'static>(
    mut compute: F,
) -> Memo<T> {
    let initial = compute();
    let signal = Signal::new(initial);
    let signal_clone = signal.clone();

    let effect_id = next_effect_id();
    EFFECTS.with(|e| {
        e.borrow_mut().insert(
            effect_id,
            EffectState {
                f: Box::new(move || {
                    let new_value = compute();
                    let changed = {
                        let current = signal_clone.value.borrow();
                        new_value != *current
                    };
                    if changed {
                        *signal_clone.value.borrow_mut() = new_value;
                        mark_dirty(signal_clone.id);
                    }
                }),
                name: "memo".to_string(),
            },
        );
    });

    // Run the effect to collect initial dependencies
    run_effect(effect_id);

    Memo { signal, effect_id }
}

/// Create an effect that runs once on mount
pub fn create_effect_once<F: FnOnce() + 'static>(_name: &str, f: F) {
    f();
}

// ── Batch Updates ──────────────────────────────────────────────────────

pub fn batch<F: FnOnce()>(f: F) {
    f();
    flush_effects();
}

// ── Hooks (for use inside #[component] functions) ──────────────────────

use crate::lifecycle::{get_hook_state, set_hook_state};
use std::any::TypeId;

/// Create a signal tied to the current component scope.
/// Returns the same signal on subsequent calls within the same component.
pub fn use_signal<T: Clone + 'static>(initial: T) -> (Signal<T>, SignalSetter<T>) {
    let key = TypeId::of::<Signal<T>>();

    // Reuse existing signal if component already created one
    if let Some(existing) = get_hook_state::<Signal<T>>(key.clone()) {
        let setter = SignalSetter {
            id: existing.id,
            value: existing.value.clone(),
        };
        return (existing, setter);
    }

    let (signal, setter) = create_signal(initial);
    set_hook_state(key, signal.clone());
    (signal, setter)
}

/// Create a memo tied to the current component scope.
pub fn use_memo<T: Clone + 'static + PartialEq, F: FnMut() -> T + 'static>(compute: F) -> Memo<T> {
    let key = TypeId::of::<Memo<T>>();

    if let Some(existing) = get_hook_state::<Memo<T>>(key.clone()) {
        return existing;
    }

    let memo = create_memo(compute);
    set_hook_state(key, memo.clone());
    memo
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

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

    #[test]
    fn test_signal_clone() {
        let (count, _) = create_signal(42);
        let count2 = count.clone();
        assert_eq!(count.get(), count2.get());
    }

    #[test]
    fn test_effect_runs_on_change() {
        let (count, set_count) = create_signal(0);
        let runs = Rc::new(Cell::new(0));
        let runs_clone = runs.clone();

        let count_clone = count.clone();
        create_effect("test", move || {
            let _ = count_clone.get();
            runs_clone.set(runs_clone.get() + 1);
        });

        assert_eq!(runs.get(), 1);

        set_count.set(1);
        assert_eq!(runs.get(), 2);
    }

    #[test]
    fn test_effect_tracks_multiple_signals() {
        let (a, set_a) = create_signal(1);
        let (b, set_b) = create_signal(2);
        let sum = Rc::new(Cell::new(0));
        let sum_clone = sum.clone();

        let a_clone = a.clone();
        let b_clone = b.clone();
        create_effect("sum", move || {
            sum_clone.set(a_clone.get() + b_clone.get());
        });

        assert_eq!(sum.get(), 3);

        set_a.set(10);
        assert_eq!(sum.get(), 12);

        set_b.set(20);
        assert_eq!(sum.get(), 30);
    }

    #[test]
    fn test_memo_derived_value() {
        let (count, set_count) = create_signal(2);
        let doubled = create_memo(move || count.get() * 2);

        assert_eq!(doubled.get(), 4);

        set_count.set(5);
        assert_eq!(doubled.get(), 10);
    }

    #[test]
    fn test_memo_chained() {
        let (x, set_x) = create_signal(1);
        let doubled = create_memo(move || x.get() * 2);
        let doubled_clone = doubled.clone();
        let quad = create_memo(move || doubled_clone.get() * 2);

        assert_eq!(quad.get(), 4);

        set_x.set(3);
        assert_eq!(quad.get(), 12);
    }

    #[test]
    fn test_batch_updates() {
        let (a, set_a) = create_signal(0);
        let (b, set_b) = create_signal(0);
        let sum = Rc::new(Cell::new(0));
        let sum_clone = sum.clone();

        let a_clone = a.clone();
        let b_clone = b.clone();
        create_effect("sum", move || {
            sum_clone.set(a_clone.get() + b_clone.get());
        });

        batch(|| {
            set_a.set(10);
            set_b.set(20);
        });

        assert_eq!(sum.get(), 30);
    }

    #[test]
    fn test_flush_effects() {
        let (count, set_count) = create_signal(0);
        let last_value = Rc::new(Cell::new(0));
        let last_clone = last_value.clone();

        let count_clone = count.clone();
        create_effect("tracker", move || {
            last_clone.set(count_clone.get());
        });

        set_count.set(5);
        assert_eq!(last_value.get(), 5);
    }

    #[test]
    fn test_use_signal_basic() {
        use crate::lifecycle::{create_component_scope, with_component};
        let id = create_component_scope();

        with_component(id, || {
            let (sig, setter) = use_signal(10);
            assert_eq!(sig.get(), 10);
            setter.set(20);
            assert_eq!(sig.get(), 20);

            // Same call returns same signal
            let (sig2, _) = use_signal(10);
            assert_eq!(sig2.get(), 20);
        });
    }

    #[test]
    fn test_use_memo_basic() {
        use crate::lifecycle::{create_component_scope, with_component};
        let id = create_component_scope();

        with_component(id, || {
            let (count, set_count) = use_signal(3);
            let memo = use_memo(move || count.get() * 2);
            assert_eq!(memo.get(), 6);

            set_count.set(5);
            assert_eq!(memo.get(), 10);
        });
    }
}
