/// Lifecycle hooks
pub struct Lifecycle;

impl Lifecycle {
    pub fn on_mount<F: FnOnce()>(callback: F) {
        // TODO: Register mount callback
        callback();
    }

    pub fn on_cleanup<F: FnOnce()>(_callback: F) {
        // TODO: Register cleanup callback
    }
}
