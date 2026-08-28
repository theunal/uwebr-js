use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Unique handle for a timer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerHandle(u64);

impl TimerHandle {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn id(self) -> u64 {
        self.0
    }
}

/// What kind of timer is this
#[derive(Debug, Clone)]
pub enum TimerKind {
    Timeout,
    Interval,
    AnimationFrame,
}

/// A pending timer entry
#[derive(Debug, Clone)]
pub struct TimerEntry {
    pub handle: TimerHandle,
    pub kind: TimerKind,
    pub fires_at: Instant,
    pub interval: Option<Duration>,
}

/// Thread-safe timer registry shared between core and app
#[derive(Clone)]
pub struct TimerRegistry {
    inner: Arc<Mutex<TimerRegistryInner>>,
}

struct TimerRegistryInner {
    next_id: u64,
    pending: HashMap<TimerHandle, TimerEntry>,
    callbacks: HashMap<TimerHandle, TimerCallback>,
    /// Animation frame callbacks (fire every frame)
    animation_frames: Vec<(TimerHandle, Arc<dyn Fn() + Send + Sync>)>,
}

/// Callback type for timers
type TimerCallback = Arc<dyn Fn() + Send + Sync>;

impl TimerRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TimerRegistryInner {
                next_id: 1,
                pending: HashMap::new(),
                callbacks: HashMap::new(),
                animation_frames: vec![],
            })),
        }
    }

    fn next_handle(&self) -> TimerHandle {
        let mut inner = self.inner.lock().unwrap();
        let id = inner.next_id;
        inner.next_id += 1;
        TimerHandle(id)
    }

    /// Schedule a timeout
    pub fn set_timeout(
        &self,
        callback: impl Fn() + Send + Sync + 'static,
        delay: Duration,
    ) -> TimerHandle {
        let handle = self.next_handle();
        let entry = TimerEntry {
            handle,
            kind: TimerKind::Timeout,
            fires_at: Instant::now() + delay,
            interval: None,
        };
        let mut inner = self.inner.lock().unwrap();
        inner.pending.insert(handle, entry);
        inner.callbacks.insert(handle, Arc::new(callback));
        handle
    }

    /// Schedule an interval
    pub fn set_interval(
        &self,
        callback: impl Fn() + Send + Sync + 'static,
        interval: Duration,
    ) -> TimerHandle {
        let handle = self.next_handle();
        let entry = TimerEntry {
            handle,
            kind: TimerKind::Interval,
            fires_at: Instant::now() + interval,
            interval: Some(interval),
        };
        let mut inner = self.inner.lock().unwrap();
        inner.pending.insert(handle, entry);
        inner.callbacks.insert(handle, Arc::new(callback));
        handle
    }

    /// Cancel a timer
    pub fn cancel(&self, handle: TimerHandle) {
        let mut inner = self.inner.lock().unwrap();
        inner.pending.remove(&handle);
        inner.callbacks.remove(&handle);
        inner.animation_frames.retain(|(h, _)| *h != handle);
    }

    /// Register an animation frame callback
    pub fn request_animation_frame(
        &self,
        callback: impl Fn() + Send + Sync + 'static,
    ) -> TimerHandle {
        let handle = self.next_handle();
        let mut inner = self.inner.lock().unwrap();
        inner.animation_frames.push((handle, Arc::new(callback)));
        handle
    }

    /// Fire all timers that are due. Returns the duration until the next timer fires.
    pub fn tick(&self) -> Option<Duration> {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let mut fired = vec![];

        // Find due timers
        for (handle, entry) in &inner.pending {
            if entry.fires_at <= now {
                fired.push(*handle);
            }
        }

        // Collect callbacks to fire (drop lock first)
        let mut to_fire: Vec<(TimerCallback, TimerKind, Option<Duration>)> = vec![];
        let mut next_fire: Option<Duration> = None;

        for handle in &fired {
            if let Some(entry) = inner.pending.get(handle) {
                let kind = entry.kind.clone();
                let interval = entry.interval;

                if let Some(cb) = inner.callbacks.get(handle) {
                    to_fire.push((cb.clone(), kind, interval));
                }

                // Reschedule intervals
                if let Some(entry) = inner.pending.get_mut(handle) {
                    if let Some(iv) = entry.interval {
                        entry.fires_at = now + iv;
                    } else {
                        // Timeout — remove after firing
                        inner.pending.remove(handle);
                        inner.callbacks.remove(handle);
                    }
                }
            }
        }

        // Calculate next wake time
        for entry in inner.pending.values() {
            let remaining = entry.fires_at.duration_since(now);
            next_fire = Some(match next_fire {
                Some(current) => current.min(remaining),
                None => remaining,
            });
        }

        drop(inner);

        // Fire callbacks outside the lock
        for (cb, _kind, _interval) in to_fire {
            cb();
        }

        next_fire
    }

    /// Fire all animation frame callbacks
    pub fn fire_animation_frames(&self) {
        let inner = self.inner.lock().unwrap();
        let frames: Vec<_> = inner
            .animation_frames
            .iter()
            .map(|(_, cb)| cb.clone())
            .collect();
        drop(inner);

        for cb in frames {
            cb();
        }
    }

    /// Check if any timers are pending
    pub fn has_pending(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !inner.pending.is_empty() || !inner.animation_frames.is_empty()
    }

    /// Get the count of pending timers
    pub fn pending_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.pending.len()
    }
}

impl Default for TimerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Convenience functions (require global registry) ──────────────

use std::sync::OnceLock;

static GLOBAL_REGISTRY: OnceLock<TimerRegistry> = OnceLock::new();

fn global_registry() -> &'static TimerRegistry {
    GLOBAL_REGISTRY.get_or_init(TimerRegistry::new)
}

/// Schedule a timeout (global)
pub fn set_timeout(callback: impl Fn() + Send + Sync + 'static, delay: Duration) -> TimerHandle {
    global_registry().set_timeout(callback, delay)
}

/// Schedule an interval (global)
pub fn set_interval(
    callback: impl Fn() + Send + Sync + 'static,
    interval: Duration,
) -> TimerHandle {
    global_registry().set_interval(callback, interval)
}

/// Cancel a timer (global)
pub fn cancel_timer(handle: TimerHandle) {
    global_registry().cancel(handle);
}

/// Request animation frame (global)
pub fn request_animation_frame(callback: impl Fn() + Send + Sync + 'static) -> TimerHandle {
    global_registry().request_animation_frame(callback)
}

/// Get the global registry
pub fn timer_registry() -> &'static TimerRegistry {
    global_registry()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_timer_handle_unique() {
        let r = TimerRegistry::new();
        let h1 = r.set_timeout(|| {}, Duration::from_millis(100));
        let h2 = r.set_timeout(|| {}, Duration::from_millis(100));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_set_timeout_not_fired() {
        let r = TimerRegistry::new();
        let _h = r.set_timeout(|| {}, Duration::from_secs(10));
        assert_eq!(r.pending_count(), 1);
    }

    #[test]
    fn test_cancel_timer() {
        let r = TimerRegistry::new();
        let h = r.set_timeout(|| {}, Duration::from_secs(10));
        r.cancel(h);
        assert_eq!(r.pending_count(), 0);
    }

    #[test]
    fn test_set_interval() {
        let r = TimerRegistry::new();
        let _h = r.set_interval(|| {}, Duration::from_millis(50));
        assert_eq!(r.pending_count(), 1);
    }

    #[test]
    fn test_request_animation_frame() {
        let r = TimerRegistry::new();
        let _h = r.request_animation_frame(|| {});
        assert!(r.has_pending());
    }

    #[test]
    fn test_tick_no_fired() {
        let r = TimerRegistry::new();
        let _h = r.set_timeout(|| {}, Duration::from_secs(10));
        let next = r.tick();
        // No timer should have fired, but we should get next wake time
        assert!(next.is_some());
    }

    #[test]
    fn test_tick_fires_expired() {
        let r = TimerRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let _h = r.set_timeout(
            move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
            Duration::from_millis(0),
        ); // Fire immediately

        r.tick();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        // Timeout should be removed after firing
        assert_eq!(r.pending_count(), 0);
    }

    #[test]
    fn test_interval_reschedules() {
        let r = TimerRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let _h = r.set_interval(
            move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
            Duration::from_millis(0),
        );

        r.tick();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        // Interval should still be pending
        assert_eq!(r.pending_count(), 1);
    }

    #[test]
    fn test_animation_frame_fires() {
        let r = TimerRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let _h = r.request_animation_frame(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        r.fire_animation_frames();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_global_registry() {
        let r = timer_registry();
        let h = r.set_timeout(|| {}, Duration::from_secs(1));
        assert_eq!(r.pending_count(), 1);
        r.cancel(h);
    }

    #[test]
    fn test_tick_returns_next_wake() {
        let r = TimerRegistry::new();
        let _h = r.set_timeout(|| {}, Duration::from_millis(100));
        let next = r.tick().unwrap();
        // Should be close to 100ms
        assert!(next.as_millis() <= 100);
    }
}
