//! Lightweight performance metrics for the render pipeline.
//!
//! These are deliberately self-contained: `uwebr-render` builds scenes only and
//! does not depend on `uwebr-html`, so the layout benchmark constructs an
//! [`Element`] tree directly rather than parsing HTML. Measurements are wall
//! clock and single-shot — good enough for a `uwebr metrics` snapshot, while
//! `criterion` (see `benches/`) handles statistically rigorous benchmarking.

use std::time::Instant;

use uwebr_core::component::{Element, NodeType, PropValue};

use crate::layout::LayoutEngine;
use crate::stylebook::StyleBook;

/// A snapshot of framework performance metrics.
#[derive(Debug, Clone)]
pub struct Metrics {
    /// Frames per second derived from the last measured frame time.
    pub fps: f64,
    /// Time to render the last frame, in milliseconds.
    pub frame_time_ms: f64,
    /// Time to parse a small stylesheet from a cold start, in milliseconds.
    pub cold_start_ms: f64,
    /// Time to lay out a 1000-node tree, in milliseconds.
    pub layout_1000_nodes_ms: f64,
    /// Resident memory estimate in bytes (0 when unavailable on this platform).
    pub memory_bytes: u64,
    /// Size of the running executable in bytes (0 when it cannot be read).
    pub binary_size_bytes: u64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            fps: 0.0,
            frame_time_ms: 0.0,
            cold_start_ms: 0.0,
            layout_1000_nodes_ms: 0.0,
            memory_bytes: 0,
            binary_size_bytes: 0,
        }
    }
}

impl Metrics {
    /// Measure every self-contained metric (everything except live FPS).
    pub fn measure_all() -> Self {
        Self {
            cold_start_ms: Self::measure_cold_start(),
            layout_1000_nodes_ms: Self::measure_layout_1000(),
            memory_bytes: Self::measure_memory(),
            binary_size_bytes: Self::measure_binary_size(),
            ..Self::default()
        }
    }

    /// Time a cold parse of a small stylesheet.
    pub fn measure_cold_start() -> f64 {
        let start = Instant::now();
        let css = ".a { width: 100px; height: 200px; background: red; }";
        let _ = StyleBook::parse(css);
        start.elapsed().as_secs_f64() * 1000.0
    }

    /// Time building + laying out a 1000-node flat tree.
    pub fn measure_layout_1000() -> f64 {
        let css = ".box { width: 10px; height: 10px; }";
        let root = build_flat_tree(1000);

        let start = Instant::now();
        if let Ok(stylebook) = StyleBook::parse(css) {
            let mut engine = LayoutEngine::new();
            if let Ok(node) = engine.build_tree(&root, &stylebook) {
                let _ = engine.compute(node, 800.0, 600.0);
            }
        }
        start.elapsed().as_secs_f64() * 1000.0
    }

    /// Best-effort resident memory estimate in bytes.
    ///
    /// Uses `sysinfo` to read the current process's memory. Returns 0 when the
    /// process cannot be found (e.g. a sandboxed CI environment), signalling
    /// "not measured" rather than a misleading number.
    pub fn measure_memory() -> u64 {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

        let pid = Pid::from_u32(std::process::id());
        let mut sys = System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().with_memory(),
        );
        sys.process(pid).map(|p| p.memory()).unwrap_or(0)
    }

    /// Size of the current executable on disk, or 0 if it cannot be determined.
    pub fn measure_binary_size() -> u64 {
        std::env::current_exe()
            .and_then(std::fs::metadata)
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// Convert a frame time in milliseconds to frames per second.
    pub fn fps_from_frame_time(frame_time_ms: f64) -> f64 {
        if frame_time_ms > 0.0 {
            1000.0 / frame_time_ms
        } else {
            0.0
        }
    }
}

/// Build a flat tree of `count` `<div class="box">` children under one root.
fn build_flat_tree(count: usize) -> Element {
    let children = (0..count)
        .map(|_| Element {
            node_type: NodeType::Element("div".to_string()),
            props: vec![("class".to_string(), PropValue::String("box".to_string()))],
            children: vec![Element::text("x")],
        })
        .collect();

    Element {
        node_type: NodeType::Element("div".to_string()),
        props: vec![],
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_cold_start_is_positive() {
        assert!(Metrics::measure_cold_start() > 0.0);
    }

    #[test]
    fn test_measure_layout_1000_is_positive() {
        assert!(Metrics::measure_layout_1000() > 0.0);
    }

    #[test]
    fn test_fps_from_frame_time() {
        assert_eq!(Metrics::fps_from_frame_time(16.0), 1000.0 / 16.0);
        assert_eq!(Metrics::fps_from_frame_time(0.0), 0.0);
        // 60 fps ≈ 16.67ms/frame.
        let fps = Metrics::fps_from_frame_time(1000.0 / 60.0);
        assert!((fps - 60.0).abs() < 0.001);
    }

    #[test]
    fn test_measure_all_fills_self_contained_fields() {
        let m = Metrics::measure_all();
        assert!(m.cold_start_ms > 0.0);
        assert!(m.layout_1000_nodes_ms > 0.0);
        // fps/frame_time are runtime-only and stay at their defaults here.
        assert_eq!(m.fps, 0.0);
        assert_eq!(m.frame_time_ms, 0.0);
    }

    #[test]
    fn test_build_flat_tree_child_count() {
        let root = build_flat_tree(1000);
        assert_eq!(root.children.len(), 1000);
    }

    #[test]
    fn test_default_metrics_are_zero() {
        let m = Metrics::default();
        assert_eq!(m.fps, 0.0);
        assert_eq!(m.memory_bytes, 0);
    }

    #[test]
    fn test_measure_memory_is_nonnegative() {
        // On a real desktop this is > 0; in a sandboxed CI the process may not
        // be found and 0 is acceptable. Either way it must not panic.
        let _mem = Metrics::measure_memory();
    }

    #[test]
    fn test_measure_all_populates_memory_field() {
        // measure_all() must route the memory probe into the struct field.
        // We can't assert a specific value (platform-dependent), but on a host
        // where the probe works the field should be non-zero.
        let m = Metrics::measure_all();
        if Metrics::measure_memory() > 0 {
            assert!(
                m.memory_bytes > 0,
                "measure_all should carry the memory reading through"
            );
        }
    }

    // ── Metrics edge-case tests ─────────────────────────────────

    #[test]
    fn render_fps_from_frame_time_very_small() {
        let fps = Metrics::fps_from_frame_time(0.001);
        assert!(
            (fps - 1_000_000.0).abs() < 1.0,
            "0.001ms frame should be ~1M fps"
        );
    }

    #[test]
    fn render_fps_from_frame_time_large() {
        let fps = Metrics::fps_from_frame_time(1000.0);
        assert_eq!(fps, 1.0, "1000ms frame should be 1 fps");
    }

    #[test]
    fn render_fps_from_frame_time_negative() {
        let fps = Metrics::fps_from_frame_time(-1.0);
        assert_eq!(fps, 0.0, "negative frame time should return 0");
    }

    #[test]
    fn render_default_metrics_fields() {
        let m = Metrics::default();
        assert_eq!(m.fps, 0.0);
        assert_eq!(m.frame_time_ms, 0.0);
        assert_eq!(m.cold_start_ms, 0.0);
        assert_eq!(m.layout_1000_nodes_ms, 0.0);
        assert_eq!(m.memory_bytes, 0);
        assert_eq!(m.binary_size_bytes, 0);
    }

    #[test]
    fn render_measure_cold_start_ms_range() {
        let ms = Metrics::measure_cold_start();
        assert!(ms > 0.0, "cold start should be positive");
        assert!(
            ms < 10_000.0,
            "cold start should complete in under 10s, got {ms}ms"
        );
    }

    #[test]
    fn render_measure_layout_1000_ms_range() {
        let ms = Metrics::measure_layout_1000();
        assert!(ms > 0.0, "layout measurement should be positive");
        assert!(
            ms < 30_000.0,
            "layout should complete in under 30s, got {ms}ms"
        );
    }

    #[test]
    fn render_build_flat_tree_various_sizes() {
        for count in [1, 10, 100, 500] {
            let root = build_flat_tree(count);
            assert_eq!(root.children.len(), count);
        }
    }

    #[test]
    fn render_measure_all_struct_has_fps_zero() {
        let m = Metrics::measure_all();
        assert_eq!(m.fps, 0.0, "measure_all should leave fps at default");
    }

    #[test]
    fn render_measure_all_struct_has_frame_time_zero() {
        let m = Metrics::measure_all();
        assert_eq!(
            m.frame_time_ms, 0.0,
            "measure_all should leave frame_time_ms at default"
        );
    }
}
