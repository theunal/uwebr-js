use std::collections::HashMap;

use uwebr_css::ast::{Keyframe, KeyframeRule};
use uwebr_css::codegen::AnimationProps;

use crate::paint::ResolvedPaint;
use crate::scene::Background;

/// Runtime state for one animation on one element.
#[derive(Debug, Clone)]
pub struct AnimationState {
    /// The animation name (matches a `@keyframes` rule).
    pub name: String,
    /// Duration in milliseconds.
    pub duration_ms: u32,
    /// Delay in milliseconds.
    pub delay_ms: u32,
    /// Iteration count (`None` = infinite).
    pub iteration_count: Option<u32>,
    /// Direction: "normal", "reverse", "alternate", "alternate-reverse".
    pub direction: String,
    /// Fill mode: "none", "forwards", "backwards", "both".
    pub fill_mode: String,
    /// Monotonic timestamp when this animation started (ms).
    pub start_ms: f64,
    /// Parsed keyframes for this animation.
    pub keyframes: Vec<Keyframe>,
}

impl AnimationState {
    /// Create a new animation state from CSS animation properties and keyframes.
    pub fn new(anim: &AnimationProps, keyframe_rule: Option<&KeyframeRule>, start_ms: f64) -> Self {
        Self {
            name: anim.name.clone(),
            duration_ms: anim.duration_ms.max(1),
            delay_ms: anim.delay_ms,
            iteration_count: anim.iteration_count,
            direction: anim.direction.clone(),
            fill_mode: anim.fill_mode.clone(),
            start_ms,
            keyframes: keyframe_rule
                .map(|k| k.keyframes.clone())
                .unwrap_or_default(),
        }
    }

    /// Returns the effective duration (including delay) in milliseconds.
    pub fn total_duration_ms(&self) -> f64 {
        self.delay_ms as f64 + self.duration_ms as f64
    }

    /// Returns the current progress (0.0..=1.0) within one iteration at the
    /// given time, accounting for delay. Returns `None` if the animation has
    /// not started yet or has finished all iterations.
    pub fn progress_at(&self, now_ms: f64) -> Option<f64> {
        let elapsed = now_ms - self.start_ms;
        if elapsed < self.delay_ms as f64 {
            return None;
        }
        let active = elapsed - self.delay_ms as f64;
        let dur = self.duration_ms as f64;
        if dur <= 0.0 {
            return None;
        }

        let iteration = (active / dur) as u32;

        if let Some(max) = self.iteration_count {
            if iteration >= max {
                return if self.fill_mode == "forwards" || self.fill_mode == "both" {
                    Some(1.0)
                } else {
                    None
                };
            }
        }

        let t = (active % dur) / dur;
        Some(self.apply_direction(t, iteration))
    }

    /// Apply direction logic to the raw 0..1 progress.
    fn apply_direction(&self, t: f64, iteration: u32) -> f64 {
        match self.direction.as_str() {
            "reverse" => 1.0 - t,
            "alternate" => {
                if iteration.is_multiple_of(2) {
                    t
                } else {
                    1.0 - t
                }
            }
            "alternate-reverse" => {
                if iteration.is_multiple_of(2) {
                    1.0 - t
                } else {
                    t
                }
            }
            _ => t, // "normal"
        }
    }

    /// Interpolate all animatable properties at the given time and apply them
    /// to the given `ResolvedPaint`.
    pub fn apply_at(&self, now_ms: f64, paint: &mut ResolvedPaint) {
        let Some(progress) = self.progress_at(now_ms) else {
            return;
        };
        if self.keyframes.is_empty() {
            return;
        }

        // Find the two bounding keyframes.
        let (from, to, local_t) = find_keyframe_pair(&self.keyframes, progress);

        // Interpolate opacity.
        if let (Some(from_opacity), Some(to_opacity)) = (from.opacity, to.opacity) {
            paint.opacity = lerp(from_opacity, to_opacity, local_t);
        }

        // Interpolate transform.
        paint.transform = lerp_transform(&from.transform, &to.transform, local_t);

        // Interpolate background color.
        if let (Some(from_bg), Some(to_bg)) = (&from.background, &to.background) {
            paint.background = Some(lerp_background(from_bg, to_bg, local_t));
        }

        // Interpolate font-size.
        if let (Some(from_fs), Some(to_fs)) = (from.font_size, to.font_size) {
            paint.font_size = lerp(from_fs, to_fs, local_t);
        }
    }
}

/// A single keyframe parsed for animation interpolation — a simplified version
/// of the CSS keyframe that carries only animatable properties.
#[derive(Debug, Clone, Default)]
pub struct AnimKeyframe {
    /// Progress point (0% = 0.0, 100% = 1.0).
    pub progress: f64,
    /// Opacity if specified.
    pub opacity: Option<f32>,
    /// Transform if specified.
    pub transform: TransformSnapshot,
    /// Background if specified.
    pub background: Option<Background>,
    /// Font size if specified.
    pub font_size: Option<f32>,
}

/// Snapshot of transform values for interpolation.
#[derive(Debug, Clone, Default)]
pub struct TransformSnapshot {
    pub translate_x: Option<f32>,
    pub translate_y: Option<f32>,
    pub rotate: Option<f32>,
    pub scale_x: Option<f32>,
    pub scale_y: Option<f32>,
}

impl From<&uwebr_css::codegen::TransformProps> for TransformSnapshot {
    fn from(tp: &uwebr_css::codegen::TransformProps) -> Self {
        Self {
            translate_x: tp.translate_x,
            translate_y: tp.translate_y,
            rotate: tp.rotate,
            scale_x: tp.scale_x,
            scale_y: tp.scale_y,
        }
    }
}

/// Parse a `Keyframe` from the CSS AST into an `AnimKeyframe`.
fn parse_keyframe(kf: &Keyframe) -> AnimKeyframe {
    let progress = parse_keyframe_selector(&kf.selector);
    let mut ak = AnimKeyframe {
        progress,
        ..Default::default()
    };

    for prop in &kf.properties {
        match prop.name.as_str() {
            "opacity" => {
                if let uwebr_css::ast::CssValue::Length(v, _) = &prop.value {
                    ak.opacity = Some(*v);
                }
            }
            "background" | "background-color" => {
                if let uwebr_css::ast::CssValue::Color(c) = &prop.value {
                    use crate::color::css_color_to_peniko;
                    ak.background = Some(Background::Solid(css_color_to_peniko(c.clone())));
                }
            }
            "font-size" => {
                if let uwebr_css::ast::CssValue::Length(v, _) = &prop.value {
                    ak.font_size = Some(*v);
                }
            }
            _ => {} // non-animatable — skip
        }
    }
    ak
}

/// Parse a keyframe selector like "0%", "50%", "from", "to" into 0.0..1.0.
fn parse_keyframe_selector(selector: &str) -> f64 {
    let s = selector.trim().to_lowercase();
    match s.as_str() {
        "from" => 0.0,
        "to" => 1.0,
        _ => {
            let pct = s.trim_end_matches('%').trim();
            pct.parse::<f64>().unwrap_or(0.0) / 100.0
        }
    }
}

/// Parse a list of `Keyframe` AST nodes into `AnimKeyframe`s, sorted by progress.
fn parse_keyframes(keyframes: &[Keyframe]) -> Vec<AnimKeyframe> {
    let mut parsed: Vec<AnimKeyframe> = keyframes.iter().map(parse_keyframe).collect();
    parsed.sort_by(|a, b| a.progress.partial_cmp(&b.progress).unwrap());
    parsed
}

/// Find the two bounding keyframes and the local interpolation factor.
fn find_keyframe_pair(keyframes: &[Keyframe], progress: f64) -> (AnimKeyframe, AnimKeyframe, f64) {
    let parsed = parse_keyframes(keyframes);
    if parsed.is_empty() {
        return (AnimKeyframe::default(), AnimKeyframe::default(), 0.0);
    }
    if parsed.len() == 1 {
        return (parsed[0].clone(), parsed[0].clone(), 0.0);
    }

    // Find the last keyframe at or before progress.
    let mut from_idx = 0;
    for (i, kf) in parsed.iter().enumerate() {
        if kf.progress <= progress {
            from_idx = i;
        }
    }

    let to_idx = (from_idx + 1).min(parsed.len() - 1);
    let from = &parsed[from_idx];
    let to = &parsed[to_idx];

    let range = to.progress - from.progress;
    let local_t = if range > 0.0 {
        ((progress - from.progress) / range).clamp(0.0, 1.0)
    } else {
        0.0
    };

    (from.clone(), to.clone(), local_t)
}

/// Linear interpolation between two f32 values.
fn lerp(a: f32, b: f32, t: f64) -> f32 {
    a + (b - a) * t as f32
}

/// Interpolate transform properties.
fn lerp_transform(
    from: &TransformSnapshot,
    to: &TransformSnapshot,
    t: f64,
) -> uwebr_css::codegen::TransformProps {
    uwebr_css::codegen::TransformProps {
        translate_x: opt_lerp(from.translate_x, to.translate_x, t),
        translate_y: opt_lerp(from.translate_y, to.translate_y, t),
        rotate: opt_lerp(from.rotate, to.rotate, t),
        scale_x: opt_lerp(from.scale_x, to.scale_x, t),
        scale_y: opt_lerp(from.scale_y, to.scale_y, t),
        skew_x: None, // skew is rarely animated
        skew_y: None,
    }
}

/// Interpolate two optional f32 values.
fn opt_lerp(a: Option<f32>, b: Option<f32>, t: f64) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(lerp(a, b, t)),
        (Some(v), None) | (None, Some(v)) => Some(v),
        (None, None) => None,
    }
}

/// Interpolate between two `Background` values.
fn lerp_background(from: &Background, to: &Background, t: f64) -> Background {
    match (from, to) {
        (Background::Solid(fc), Background::Solid(tc)) => Background::Solid(lerp_color(fc, tc, t)),
        // For gradients, just crossfade by choosing based on progress.
        (a, _) if t < 0.5 => a.clone(),
        (_, b) => b.clone(),
    }
}

/// Interpolate two peniko colors.
fn lerp_color(
    from: &vello::peniko::Color,
    to: &vello::peniko::Color,
    t: f64,
) -> vello::peniko::Color {
    let frgba = from.to_rgba8();
    let trgba = to.to_rgba8();
    let lerp_u8 = |a: u8, b: u8| -> u8 { (a as f32 + (b as f32 - a as f32) * t as f32) as u8 };

    vello::peniko::Color::from_rgba8(
        lerp_u8(frgba.r, trgba.r),
        lerp_u8(frgba.g, trgba.g),
        lerp_u8(frgba.b, trgba.b),
        lerp_u8(frgba.a, trgba.a),
    )
}

/// Container holding active animations for the scene.
#[derive(Debug, Default)]
pub struct AnimationTracker {
    /// Maps node_id → list of active animation states.
    active: HashMap<usize, Vec<AnimationState>>,
}

impl AnimationTracker {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
        }
    }

    /// Start an animation on a node if not already running.
    pub fn start_if_needed(
        &mut self,
        node_id: usize,
        anim: &AnimationProps,
        keyframes: Option<&KeyframeRule>,
        now_ms: f64,
    ) {
        let entry = self.active.entry(node_id).or_default();
        // Don't restart if same-name animation is already running.
        if entry.iter().any(|a| a.name == anim.name) {
            return;
        }
        entry.push(AnimationState::new(anim, keyframes, now_ms));
    }

    /// Apply all active animations for a node to its paint at the given time.
    pub fn apply(&self, node_id: usize, now_ms: f64, paint: &mut ResolvedPaint) {
        if let Some(states) = self.active.get(&node_id) {
            for state in states {
                state.apply_at(now_ms, paint);
            }
        }
    }

    /// Remove finished animations. Returns true if any were removed.
    pub fn prune(&mut self, now_ms: f64) -> bool {
        let mut removed = false;
        self.active.retain(|_, states| {
            let before = states.len();
            states.retain(|s| {
                s.progress_at(now_ms).is_some()
                    || (s.fill_mode == "forwards" || s.fill_mode == "both")
            });
            if states.len() < before {
                removed = true;
            }
            !states.is_empty()
        });
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwebr_css::ast::{CssProperty, CssValue, Keyframe};
    use uwebr_css::codegen::TransformProps;

    fn make_keyframe(name: &str, opacity: Option<f32>) -> Keyframe {
        let mut props = vec![];
        if let Some(o) = opacity {
            props.push(CssProperty {
                name: "opacity".into(),
                value: CssValue::Length(o, uwebr_css::ast::LengthUnit::Px),
                important: false,
            });
        }
        Keyframe {
            selector: name.into(),
            properties: props,
        }
    }

    #[test]
    fn test_parse_keyframe_selector_from() {
        assert_eq!(parse_keyframe_selector("from"), 0.0);
        assert_eq!(parse_keyframe_selector("FROM"), 0.0);
    }

    #[test]
    fn test_parse_keyframe_selector_to() {
        assert_eq!(parse_keyframe_selector("to"), 1.0);
        assert_eq!(parse_keyframe_selector("TO"), 1.0);
    }

    #[test]
    fn test_parse_keyframe_selector_percent() {
        assert!((parse_keyframe_selector("50%") - 0.5).abs() < f64::EPSILON);
        assert!((parse_keyframe_selector("0%") - 0.0).abs() < f64::EPSILON);
        assert!((parse_keyframe_selector("100%") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_lerp_basic() {
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < f32::EPSILON);
        assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < f32::EPSILON);
        assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_opt_lerp_both_some() {
        let result = opt_lerp(Some(0.0), Some(10.0), 0.5).unwrap();
        assert!((result - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_opt_lerp_one_none() {
        assert_eq!(opt_lerp(Some(5.0), None, 0.5), Some(5.0));
        assert_eq!(opt_lerp(None, Some(5.0), 0.5), Some(5.0));
        assert_eq!(opt_lerp(None, None, 0.5), None);
    }

    #[test]
    fn test_progress_at_before_delay() {
        let anim = AnimationProps {
            name: "fade".into(),
            duration_ms: 1000,
            delay_ms: 200,
            ..Default::default()
        };
        let state = AnimationState::new(&anim, None, 0.0);
        // At t=100, still in delay period.
        assert_eq!(state.progress_at(100.0), None);
    }

    #[test]
    fn test_progress_at_midway() {
        let anim = AnimationProps {
            name: "fade".into(),
            duration_ms: 1000,
            delay_ms: 0,
            ..Default::default()
        };
        let state = AnimationState::new(&anim, None, 0.0);
        let p = state.progress_at(500.0).unwrap();
        assert!((p - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_progress_at_finished_no_fill() {
        let anim = AnimationProps {
            name: "fade".into(),
            duration_ms: 500,
            iteration_count: Some(1),
            fill_mode: "none".into(),
            ..Default::default()
        };
        let state = AnimationState::new(&anim, None, 0.0);
        assert_eq!(state.progress_at(600.0), None);
    }

    #[test]
    fn test_progress_at_finished_forwards_fill() {
        let anim = AnimationProps {
            name: "fade".into(),
            duration_ms: 500,
            iteration_count: Some(1),
            fill_mode: "forwards".into(),
            ..Default::default()
        };
        let state = AnimationState::new(&anim, None, 0.0);
        assert_eq!(state.progress_at(600.0), Some(1.0));
    }

    #[test]
    fn test_direction_reverse() {
        let anim = AnimationProps {
            name: "fade".into(),
            duration_ms: 1000,
            direction: "reverse".into(),
            ..Default::default()
        };
        let state = AnimationState::new(&anim, None, 0.0);
        // At 50% through, reverse should give 0.5.
        let p = state.progress_at(500.0).unwrap();
        assert!((p - 0.5).abs() < 0.01);
        // At 0%, reverse should give 1.0.
        let p = state.progress_at(0.0).unwrap();
        assert!((p - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_direction_alternate() {
        let anim = AnimationProps {
            name: "fade".into(),
            duration_ms: 500,
            iteration_count: Some(2),
            direction: "alternate".into(),
            ..Default::default()
        };
        let state = AnimationState::new(&anim, None, 0.0);
        // First iteration at 50%: normal → 0.5.
        let p = state.progress_at(250.0).unwrap();
        assert!((p - 0.5).abs() < 0.02);
    }

    #[test]
    fn test_find_keyframe_pair_single() {
        let keyframes = vec![make_keyframe("50%", Some(0.5))];
        let (from, to, t) = find_keyframe_pair(&keyframes, 0.5);
        assert_eq!(from.progress, 0.5);
        assert_eq!(to.progress, 0.5);
        assert!((t - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_find_keyframe_pair_two() {
        let keyframes = vec![
            make_keyframe("from", Some(1.0)),
            make_keyframe("to", Some(0.0)),
        ];
        let (from, to, t) = find_keyframe_pair(&keyframes, 0.25);
        assert_eq!(from.progress, 0.0);
        assert_eq!(to.progress, 1.0);
        assert!((t - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_animation_state_apply_opacity() {
        let keyframes = vec![
            make_keyframe("from", Some(1.0)),
            make_keyframe("to", Some(0.0)),
        ];
        let anim = AnimationProps {
            name: "fade".into(),
            duration_ms: 1000,
            ..Default::default()
        };

        let kr = KeyframeRule {
            name: "fade".into(),
            keyframes: keyframes.clone(),
        };

        let state = AnimationState::new(&anim, Some(&kr), 0.0);
        let mut paint = ResolvedPaint::default();
        // At t=500 (50%), opacity should interpolate from 1.0 to 0.0.
        state.apply_at(500.0, &mut paint);
        assert!((paint.opacity - 0.5).abs() < 0.02);
    }

    #[test]
    fn test_tracker_start_and_apply() {
        let mut tracker = AnimationTracker::new();
        let anim = AnimationProps {
            name: "fade".into(),
            duration_ms: 1000,
            ..Default::default()
        };
        let kr = KeyframeRule {
            name: "fade".into(),
            keyframes: vec![
                make_keyframe("from", Some(1.0)),
                make_keyframe("to", Some(0.0)),
            ],
        };

        tracker.start_if_needed(0, &anim, Some(&kr), 0.0);
        // Second call should not restart.
        tracker.start_if_needed(0, &anim, Some(&kr), 100.0);

        let mut paint = ResolvedPaint::default();
        tracker.apply(0, 500.0, &mut paint);
        assert!((paint.opacity - 0.5).abs() < 0.02);
    }

    #[test]
    fn test_prune_removes_finished() {
        let mut tracker = AnimationTracker::new();
        let anim = AnimationProps {
            name: "fade".into(),
            duration_ms: 500,
            iteration_count: Some(1),
            fill_mode: "none".into(),
            ..Default::default()
        };
        let kr = KeyframeRule {
            name: "fade".into(),
            keyframes: vec![
                make_keyframe("from", Some(1.0)),
                make_keyframe("to", Some(0.0)),
            ],
        };

        tracker.start_if_needed(0, &anim, Some(&kr), 0.0);
        assert!(!tracker.active.is_empty());
        // After animation finishes with fill_mode="none", it should be pruned.
        tracker.prune(600.0);
        assert!(tracker.active.is_empty());
    }
}
