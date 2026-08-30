use parley::style::{FontFamily, StyleProperty};
use parley::{Alignment, AlignmentOptions, FontContext, Layout, LayoutContext};

/// Ratio used to estimate glyph advance when no system font is available.
///
/// `FontContext` enumerates system fonts; in headless environments (CI
/// containers, minimal images) it can come back empty and parley then produces
/// a zero-sized layout. Without a fallback the node would collapse to 0x0 and
/// silently disappear from the scene, so we approximate a monospace-ish box.
const FALLBACK_ADVANCE_RATIO: f32 = 0.55;
/// Line height multiplier for the no-font fallback.
const FALLBACK_LINE_HEIGHT_RATIO: f32 = 1.25;

/// Text renderer using parley for text layout.
///
/// Owns the font and layout contexts, so it must be created once and reused —
/// `FontContext::new()` enumerates the system font collection.
pub struct TextRenderer {
    font_context: FontContext,
    layout_context: LayoutContext<()>,
}

impl TextRenderer {
    pub fn new() -> Self {
        Self {
            font_context: FontContext::new(),
            layout_context: LayoutContext::new(),
        }
    }

    /// Build a line-broken, aligned parley `Layout` ready for measuring or drawing.
    ///
    /// `max_advance` is the wrapping width; `None` means "do not wrap".
    pub fn layout_text(
        &mut self,
        content: &str,
        font_size: f32,
        font_family: Option<&str>,
        max_advance: Option<f32>,
    ) -> Layout<()> {
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, content, 1.0, true);

        builder.push_default(StyleProperty::FontSize(font_size));
        if let Some(family) = font_family {
            // parley parses CSS-style lists ("system-ui, sans-serif") from Source.
            builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
                std::borrow::Cow::Borrowed(family),
            )));
        }

        let mut layout = builder.build(content);
        // Line breaking is required before lines()/align() return anything.
        layout.break_all_lines(max_advance);
        layout.align(Alignment::Start, AlignmentOptions::default());
        layout
    }

    /// Measure text dimensions, falling back to an estimate when no font resolved.
    pub fn measure(
        &mut self,
        content: &str,
        font_size: f32,
        font_family: Option<&str>,
        max_advance: Option<f32>,
    ) -> (f32, f32) {
        if content.is_empty() {
            return (0.0, 0.0);
        }

        let layout = self.layout_text(content, font_size, font_family, max_advance);
        let (w, h) = (layout.width(), layout.height());

        if w > 0.0 && h > 0.0 {
            (w, h)
        } else {
            estimate_text_size(content, font_size, max_advance)
        }
    }

    /// Measure dimensions of an already-built layout (kept for callers holding a Layout).
    pub fn measure_text(&self, layout: &Layout<()>) -> (f32, f32) {
        let mut max_width: f32 = 0.0;
        let mut total_height: f32 = 0.0;

        for line in layout.lines() {
            let metrics = line.metrics();
            total_height += metrics.line_height;
            max_width = max_width.max(metrics.advance);
        }

        (max_width, total_height)
    }
}

/// Font-less size estimate: character count times a fraction of the font size.
pub fn estimate_text_size(content: &str, font_size: f32, max_advance: Option<f32>) -> (f32, f32) {
    if content.is_empty() {
        return (0.0, 0.0);
    }

    let char_width = font_size * FALLBACK_ADVANCE_RATIO;
    let line_height = font_size * FALLBACK_LINE_HEIGHT_RATIO;
    let chars = content.chars().count() as f32;
    let natural_width = chars * char_width;

    match max_advance {
        Some(limit) if limit > 0.0 && natural_width > limit => {
            let per_line = (limit / char_width).floor().max(1.0);
            let lines = (chars / per_line).ceil().max(1.0);
            (limit, lines * line_height)
        }
        _ => (natural_width, line_height),
    }
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_renderer_creation() {
        let tr = TextRenderer::new();
        let _ = &tr.font_context;
    }

    #[test]
    fn test_layout_text() {
        let mut tr = TextRenderer::new();
        let layout = tr.layout_text("Hello World", 16.0, None, None);
        // parley may return 0 lines if no system fonts are available
        let _line_count = layout.lines().count();
    }

    #[test]
    fn test_measure_text() {
        let mut tr = TextRenderer::new();
        let layout = tr.layout_text("Hello", 16.0, None, None);
        let (w, h) = tr.measure_text(&layout);
        // Values depend on font availability
        assert!(w >= 0.0);
        assert!(h >= 0.0);
    }

    #[test]
    fn test_measure_empty_text() {
        let mut tr = TextRenderer::new();
        let (w, h) = tr.measure("", 16.0, None, None);
        assert_eq!((w, h), (0.0, 0.0));
    }

    #[test]
    fn test_measure_returns_positive_size_for_text() {
        // This must hold with or without system fonts — the fallback covers the
        // no-font case, otherwise text nodes collapse to 0x0 and vanish.
        let mut tr = TextRenderer::new();
        let (w, h) = tr.measure("Hello from uwebr!", 16.0, None, None);
        assert!(w > 0.0, "expected non-zero width, got {w}");
        assert!(h > 0.0, "expected non-zero height, got {h}");
    }

    #[test]
    fn test_measure_scales_with_font_size() {
        let mut tr = TextRenderer::new();
        let (_, small) = tr.measure("Hello", 12.0, None, None);
        let (_, large) = tr.measure("Hello", 48.0, None, None);
        assert!(large > small, "48px text should be taller than 12px");
    }

    #[test]
    fn test_measure_longer_text_is_wider() {
        let mut tr = TextRenderer::new();
        let (short, _) = tr.measure("Hi", 16.0, None, None);
        let (long, _) = tr.measure("Hi there, this is much longer", 16.0, None, None);
        assert!(long > short);
    }

    #[test]
    fn test_estimate_respects_max_advance() {
        let (w, h) = estimate_text_size("aaaaaaaaaaaaaaaaaaaa", 10.0, Some(20.0));
        assert_eq!(w, 20.0, "width clamped to max_advance");
        assert!(h > 10.0, "wrapped text occupies multiple lines");
    }

    #[test]
    fn test_estimate_empty() {
        assert_eq!(estimate_text_size("", 16.0, None), (0.0, 0.0));
    }

    #[test]
    fn test_layout_with_font_family() {
        let mut tr = TextRenderer::new();
        // Must not panic on a CSS-style family list.
        let _ = tr.measure("Hello", 16.0, Some("system-ui, sans-serif"), None);
    }

    // ── Text edge-case tests ────────────────────────────────────

    #[test]
    fn render_measure_single_char() {
        let mut tr = TextRenderer::new();
        let (w, h) = tr.measure("X", 16.0, None, None);
        assert!(w > 0.0, "single char should have positive width");
        assert!(h > 0.0, "single char should have positive height");
    }

    #[test]
    fn render_measure_very_large_font() {
        let mut tr = TextRenderer::new();
        let (_, h) = tr.measure("Hello", 200.0, None, None);
        assert!(
            h > 100.0,
            "200px font should produce tall text, got height {h}"
        );
    }

    #[test]
    fn render_measure_max_advance_wrapping() {
        let mut tr = TextRenderer::new();
        let (_, h1) = tr.measure("abcdefghij", 16.0, None, Some(50.0));
        let (_, h2) = tr.measure("abcdefghij", 16.0, None, None);
        assert!(
            h1 >= h2,
            "wrapped text should have at least as much height as single-line, h1={h1}, h2={h2}"
        );
    }

    #[test]
    fn render_estimate_text_size_zero_font() {
        let (w, h) = estimate_text_size("abc", 0.0, None);
        assert_eq!(w, 0.0, "zero font size should produce zero width");
        assert_eq!(h, 0.0, "zero font size should produce zero height");
    }

    #[test]
    fn render_estimate_max_advance_one_char_per_line() {
        let (w, h) = estimate_text_size("abc", 16.0, Some(5.0));
        assert_eq!(w, 5.0, "width should be clamped");
        assert!(h > 16.0 * 1.25, "should wrap to multiple lines");
    }

    #[test]
    fn render_measure_multiline_content() {
        let mut tr = TextRenderer::new();
        let content = "Line 1\nLine 2\nLine 3";
        let (w, h) = tr.measure(content, 16.0, None, None);
        assert!(w > 0.0, "multiline text should have positive width");
        assert!(
            h > 16.0 * 1.25,
            "multiline text should be taller than single line"
        );
    }

    // ── Quality tests (test_q_*) ────────────────────────────────

    #[test]
    fn test_q_text_measure_empty_string() {
        let mut tr = TextRenderer::new();
        let (w, h) = tr.measure("", 16.0, None, None);
        assert_eq!((w, h), (0.0, 0.0), "empty string must measure 0x0");
    }

    #[test]
    fn test_q_text_measure_wrap_narrow() {
        let mut tr = TextRenderer::new();
        let long = "abcdefghij";
        let (_w_no_wrap, h_no_wrap) = tr.measure(long, 16.0, None, None);
        let (_w_wrap, h_wrap) = tr.measure(long, 16.0, None, Some(30.0));
        // Wrapping with a narrow max_advance should produce more height
        // because the text breaks into multiple lines.
        assert!(
            h_wrap >= h_no_wrap,
            "wrapped text should have >= height: wrap={h_wrap}, nowrap={h_no_wrap}"
        );
    }

    #[test]
    fn test_q_stress_large_text_10000_chars() {
        let mut tr = TextRenderer::new();
        let text: String = "a".repeat(10000);
        let (w, h) = tr.measure(&text, 16.0, None, None);
        assert!(w > 0.0, "10000 chars must have positive width");
        assert!(h > 0.0, "10000 chars must have positive height");
    }
}
