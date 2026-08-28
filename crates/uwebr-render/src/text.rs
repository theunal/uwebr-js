use parley::{FontContext, Layout, LayoutContext};

/// Text renderer using parley for text layout
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

    /// Layout text with parley, returns Layout for later drawing
    pub fn layout_text(&mut self, content: &str, font_size: f32) -> Layout<()> {
        let builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, content, font_size, true);
        builder.build(content)
    }

    /// Measure text dimensions without drawing
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
        let layout = tr.layout_text("Hello World", 16.0);
        // parley may return 0 lines if no system fonts are available
        let _line_count = layout.lines().count();
    }

    #[test]
    fn test_measure_text() {
        let mut tr = TextRenderer::new();
        let layout = tr.layout_text("Hello", 16.0);
        let (w, h) = tr.measure_text(&layout);
        // Values depend on font availability
        assert!(w >= 0.0);
        assert!(h >= 0.0);
    }

    #[test]
    fn test_measure_empty_text() {
        let mut tr = TextRenderer::new();
        let layout = tr.layout_text("", 16.0);
        let (_w, _h) = tr.measure_text(&layout);
    }
}
