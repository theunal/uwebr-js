use std::collections::HashMap;
use uwebr_css::codegen::TransformProps;
use vello::kurbo::{Affine, Rect, RoundedRect, Stroke};
use vello::peniko::{self, color::palette, Fill};

use crate::color::css_color_to_peniko;
use crate::scene::{
    Background, RenderNode, RenderNodeKind, RenderScene, RenderStyle, TextOverflow, Visibility,
};
use crate::text::TextRenderer;

/// Scroll state passed from the pipeline.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset_x: f32,
    pub offset_y: f32,
}

/// Builds a vello Scene from a RenderScene.
///
/// Owns a [`TextRenderer`] because glyph runs need a live parley
/// `FontContext`; constructing one per frame would re-enumerate the system
/// font collection.
pub struct SceneBuilder {
    text: TextRenderer,
}

impl SceneBuilder {
    pub fn new() -> Self {
        Self {
            text: TextRenderer::new(),
        }
    }

    /// Mutable access to the text renderer for text measurement queries.
    pub fn text_renderer(&mut self) -> &mut TextRenderer {
        &mut self.text
    }

    /// Build a vello::Scene from positioned render nodes.
    pub fn build(&mut self, scene: &RenderScene, width: u32, height: u32) -> vello::Scene {
        let empty: HashMap<usize, ScrollState> = HashMap::new();
        self.build_with_scroll(scene, width, height, &empty)
    }

    /// Build a vello::Scene with scroll states applied.
    pub fn build_with_scroll(
        &mut self,
        scene: &RenderScene,
        width: u32,
        height: u32,
        scroll_states: &HashMap<usize, ScrollState>,
    ) -> vello::Scene {
        let mut vello_scene = vello::Scene::new();

        // Background fill for the entire surface
        vello_scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            palette::css::BLACK,
            None,
            &Rect::new(0.0, 0.0, width as f64, height as f64),
        );

        // Sort nodes by z-index (stable sort preserves tree order for equal z).
        // Negative z falls behind, positive z paints on top.
        let mut sorted_nodes: Vec<&RenderNode> = scene.nodes().iter().collect();
        sorted_nodes.sort_by_key(|n| n.style.z_index);

        for node in sorted_nodes {
            let scroll = scroll_states
                .get(&node.node_id)
                .cloned()
                .unwrap_or_default();
            self.draw_node(&mut vello_scene, node, &scroll);
        }

        vello_scene
    }

    /// Convenience wrapper that builds its own text renderer.
    ///
    /// Prefer [`SceneBuilder::build`] on a long-lived instance in render loops.
    pub fn build_scene(scene: &RenderScene, width: u32, height: u32) -> vello::Scene {
        Self::new().build(scene, width, height)
    }

    /// Draw a single node into the vello scene
    fn draw_node(&mut self, scene: &mut vello::Scene, node: &RenderNode, scroll: &ScrollState) {
        let x = node.layout.x as f64;
        let y = node.layout.y as f64;
        let w = node.layout.width as f64;
        let h = node.layout.height as f64;

        if w <= 0.0 || h <= 0.0 {
            return;
        }

        // `visibility: hidden` renders nothing but still reserves layout space.
        if node.style.visibility == Visibility::Hidden {
            return;
        }

        let tx = Self::transform_to_affine(&node.transform, x, y);

        // Push transform layer if needed
        if tx != Affine::IDENTITY {
            scene.push_layer(
                Fill::NonZero,
                peniko::Compose::SrcOver,
                1.0,
                tx,
                &Rect::new(x, y, x + w, y + h),
            );
        }

        // Push opacity layer if needed
        if node.style.opacity < 1.0 {
            scene.push_layer(
                Fill::NonZero,
                peniko::Compose::SrcOver,
                node.style.opacity,
                Affine::IDENTITY,
                &Rect::new(x, y, x + w, y + h),
            );
        }

        // `overflow: hidden` clips descendants to this node's box.
        // `overflow: scroll` clips + offsets children by scroll position.
        let scroll_active = node.style.overflow_scroll_x || node.style.overflow_scroll_y;
        if node.style.overflow_hidden || scroll_active {
            scene.push_clip_layer(
                Fill::NonZero,
                Affine::IDENTITY,
                &Rect::new(x, y, x + w, y + h),
            );
        }

        // For scroll containers, push a translation layer with the scroll offset.
        let has_scroll_offset = scroll_active && (scroll.offset_x != 0.0 || scroll.offset_y != 0.0);
        if has_scroll_offset {
            scene.push_layer(
                Fill::NonZero,
                peniko::Compose::SrcOver,
                1.0,
                Affine::translate((-scroll.offset_x as f64, -scroll.offset_y as f64)),
                &Rect::new(0.0, 0.0, w + 5000.0, h + 5000.0),
            );
        }

        // Draw box-shadow BEFORE the node (shadows appear behind)
        if !node.box_shadow.is_empty() {
            Self::draw_box_shadow(scene, &node.box_shadow, x, y, w, h);
        }

        // Draw based on node kind
        match &node.kind {
            RenderNodeKind::Rect => {
                Self::draw_rect(scene, &node.style, x, y, w, h);
            }
            RenderNodeKind::RoundRect { radius } => {
                Self::draw_round_rect(scene, &node.style, x, y, w, h, *radius as f64);
            }
            RenderNodeKind::Text {
                content,
                font_size,
                color,
                font_family,
                font_weight,
                font_style,
                text_decoration,
            } => {
                self.draw_text(
                    scene,
                    content,
                    *font_size,
                    *color,
                    font_family.as_deref(),
                    x,
                    y,
                    w,
                    &node.style.text_overflow,
                    node.style.text_align.as_deref(),
                    node.style.line_height,
                    node.style.letter_spacing,
                    font_weight.as_deref(),
                    font_style.as_deref(),
                    text_decoration.as_deref(),
                );
            }
            RenderNodeKind::Image {
                data,
                width,
                height,
            } => {
                self.draw_image(scene, data, *width, *height, x, y, w, h);
            }
            RenderNodeKind::Container => {
                if node.style.background.is_some() {
                    if node.style.border_radius > 0.0 {
                        Self::draw_round_rect(
                            scene,
                            &node.style,
                            x,
                            y,
                            w,
                            h,
                            node.style.border_radius as f64,
                        );
                    } else {
                        Self::draw_rect(scene, &node.style, x, y, w, h);
                    }
                }
            }
            RenderNodeKind::Input {
                value,
                font_size,
                color,
                font_family,
                caret,
                selection,
                focused,
                caret_visible,
                placeholder,
            } => {
                // Background/border for the input come from the container style
                // (drawn below via border); here we draw text/caret/selection.
                if node.style.background.is_some() {
                    if node.style.border_radius > 0.0 {
                        Self::draw_round_rect(
                            scene,
                            &node.style,
                            x,
                            y,
                            w,
                            h,
                            node.style.border_radius as f64,
                        );
                    } else {
                        Self::draw_rect(scene, &node.style, x, y, w, h);
                    }
                }
                self.draw_input(
                    scene,
                    value,
                    *font_size,
                    *color,
                    font_family.as_deref(),
                    x,
                    y,
                    w,
                    h,
                    *caret,
                    *selection,
                    *focused,
                    *caret_visible,
                    placeholder.as_deref(),
                );
            }
            RenderNodeKind::Toggle {
                checked,
                radio,
                color,
            } => {
                // Draw box/circle background from style, then the mark.
                if node.style.background.is_some() {
                    if *radio {
                        Self::draw_round_rect(scene, &node.style, x, y, w, h, (w / 2.0).min(h / 2.0));
                    } else if node.style.border_radius > 0.0 {
                        Self::draw_round_rect(
                            scene,
                            &node.style,
                            x,
                            y,
                            w,
                            h,
                            node.style.border_radius as f64,
                        );
                    } else {
                        Self::draw_rect(scene, &node.style, x, y, w, h);
                    }
                }
                if *checked {
                    if *radio {
                        Self::draw_radio_dot(scene, x, y, w, h, *color);
                    } else {
                        Self::draw_checkmark(scene, x, y, w, h, *color);
                    }
                }
            }
        }

        // Draw border if present
        if let Some(ref border) = node.style.border {
            Self::draw_border(scene, x, y, w, h, border.width as f64, border.color);
        }

        // Pop scroll offset layer
        if has_scroll_offset {
            scene.pop_layer();
        }

        // Pop clip layer (hidden or scroll)
        if node.style.overflow_hidden || scroll_active {
            scene.pop_layer();
        }

        // Pop opacity layer
        if node.style.opacity < 1.0 {
            scene.pop_layer();
        }

        // Pop transform layer
        if tx != Affine::IDENTITY {
            scene.pop_layer();
        }
    }

    /// Convert `TransformProps` into a vello `Affine` around the element's origin.
    fn transform_to_affine(props: &TransformProps, x: f64, y: f64) -> Affine {
        if props.is_empty() {
            return Affine::IDENTITY;
        }
        // Build the transform around the element's own top-left corner.
        let cx = x + 0.0; // center-x for rotation = left edge
        let cy = y + 0.0; // center-y for rotation = top edge

        // Sequence: translate → rotate → scale → skew
        // translate
        let tx_val = props.translate_x.unwrap_or(0.0) as f64;
        let ty_val = props.translate_y.unwrap_or(0.0) as f64;
        let mut aff = Affine::translate((tx_val, ty_val));

        // rotate (convert degrees → radians)
        if let Some(deg) = props.rotate {
            let rad = (deg as f64).to_radians();
            aff *= Affine::rotate_about(rad, (cx, cy));
        }

        // scale
        let sx = props.scale_x.unwrap_or(1.0) as f64;
        let sy = props.scale_y.unwrap_or(1.0) as f64;
        if sx != 1.0 || sy != 1.0 {
            let scale = Affine::translate((cx, cy))
                * Affine::new([sx, 0.0, 0.0, sy, 0.0, 0.0])
                * Affine::translate((-cx, -cy));
            aff *= scale;
        }

        // skew
        let skew_x_deg = props.skew_x.unwrap_or(0.0) as f64;
        let skew_y_deg = props.skew_y.unwrap_or(0.0) as f64;
        if skew_x_deg != 0.0 || skew_y_deg != 0.0 {
            let kx = skew_x_deg.to_radians().tan();
            let ky = skew_y_deg.to_radians().tan();
            aff *= Affine::new([1.0, ky, kx, 1.0, 0.0, 0.0]);
        }

        aff
    }

    /// Lay out the string with parley and encode its glyph runs into the scene.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn draw_text(
        &mut self,
        scene: &mut vello::Scene,
        content: &str,
        font_size: f32,
        color: peniko::Color,
        font_family: Option<&str>,
        x: f64,
        y: f64,
        width: f64,
        text_overflow: &TextOverflow,
        text_align: Option<&str>,
        line_height: Option<f32>,
        letter_spacing: Option<f32>,
        font_weight: Option<&str>,
        font_style: Option<&str>,
        text_decoration: Option<&str>,
    ) {
        if content.trim().is_empty() {
            return;
        }

        // Ellipsis truncates to a single line; other modes keep the full string
        // and rely on the layout box (plus any overflow clip) to bound it.
        let display_content = if *text_overflow == TextOverflow::Ellipsis && width > 0.0 {
            self.truncate_with_ellipsis(content, font_size, font_family, width)
        } else {
            content.to_string()
        };

        // Wrap to the box the layout engine assigned to this node. Ellipsis
        // produces a single line, so wrapping is disabled in that case.
        let max_advance = if *text_overflow == TextOverflow::Ellipsis {
            None
        } else if width > 0.0 {
            Some(width as f32)
        } else {
            None
        };
        let layout = self.text.layout_text(
            &display_content,
            font_size,
            font_family,
            max_advance,
            text_align,
            line_height,
            letter_spacing,
            font_weight,
            font_style,
            text_decoration,
        );

        let mut cursor_y: f32 = 0.0;
        for line in layout.lines() {
            let metrics = line.metrics();

            // Apply line-height override: if specified, override the line advance.
            let effective_line_height = line_height
                .map(|lh| font_size * lh)
                .unwrap_or(metrics.line_height);

            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let run = glyph_run.run();
                let run_x = x;
                let run_y = y + cursor_y as f64;

                scene
                    .draw_glyphs(run.font())
                    .font_size(run.font_size())
                    .brush(color)
                    .transform(Affine::translate((run_x, run_y)))
                    .normalized_coords(run.normalized_coords())
                    .draw(
                        Fill::NonZero,
                        glyph_run.positioned_glyphs().map(|g| vello::Glyph {
                            id: g.id,
                            x: g.x,
                            y: g.y,
                        }),
                    );
            }

            cursor_y += effective_line_height;
        }
    }

    /// Total advance width of a laid-out string on its first line.
    fn measure_advance(&mut self, content: &str, font_size: f32, font_family: Option<&str>) -> f32 {
        let layout = self.text.layout_text(
            content,
            font_size,
            font_family,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        layout
            .lines()
            .flat_map(|l| l.items())
            .filter_map(|item| {
                if let parley::PositionedLayoutItem::GlyphRun(run) = item {
                    Some(run.glyphs().map(|g| g.advance).sum::<f32>())
                } else {
                    None
                }
            })
            .sum()
    }

    /// Shorten `content` so it plus an ellipsis fits within `max_width`.
    ///
    /// Returns the original string untouched when it already fits. Character-by-
    /// character measurement is O(n) in layouts but fine for the short single
    /// lines ellipsis targets.
    fn truncate_with_ellipsis(
        &mut self,
        content: &str,
        font_size: f32,
        font_family: Option<&str>,
        max_width: f64,
    ) -> String {
        let total_width = self.measure_advance(content, font_size, font_family);
        if total_width <= max_width as f32 {
            return content.to_string();
        }

        let ellipsis = "…";
        let ellipsis_width = self.measure_advance(ellipsis, font_size, font_family);
        let available = max_width as f32 - ellipsis_width;
        if available <= 0.0 {
            return ellipsis.to_string();
        }

        let mut truncated = String::new();
        let mut width_so_far = 0.0f32;
        for ch in content.chars() {
            let char_width = self.measure_advance(&ch.to_string(), font_size, font_family);
            if width_so_far + char_width > available {
                break;
            }
            truncated.push(ch);
            width_so_far += char_width;
        }

        format!("{truncated}{ellipsis}")
    }

    /// Decode an encoded image (PNG/JPEG) and paint it into the node's box.
    ///
    /// Invalid or unsupported data is silently skipped so a broken `src` cannot
    /// crash the frame. The image is scaled non-uniformly to fill the box.
    #[allow(clippy::too_many_arguments)]
    fn draw_image(
        &mut self,
        scene: &mut vello::Scene,
        data: &[u8],
        _img_width: u32,
        _img_height: u32,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) {
        let img = match image::load_from_memory(data) {
            Ok(img) => img,
            Err(_) => return,
        };

        let rgba = img.to_rgba8();
        let (iw, ih) = rgba.dimensions();
        if iw == 0 || ih == 0 {
            return;
        }

        let image_data = peniko::ImageData {
            data: peniko::Blob::from(rgba.into_raw()),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: iw,
            height: ih,
        };

        let transform =
            Affine::translate((x, y)) * Affine::scale_non_uniform(w / iw as f64, h / ih as f64);
        scene.draw_image(&image_data, transform);
    }

    /// Draw box-shadows behind a node's box.
    fn draw_box_shadow(
        scene: &mut vello::Scene,
        shadows: &[uwebr_css::codegen::BoxShadow],
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) {
        for shadow in shadows {
            let ox = shadow.offset_x as f64;
            let oy = shadow.offset_y as f64;
            let r = shadow.blur as f64 * 0.5;
            let sp = shadow.spread as f64;

            let sx = x + ox - sp;
            let sy = y + oy - sp;
            let sw = w + sp * 2.0;
            let sh = h + sp * 2.0;

            if sw <= 0.0 || sh <= 0.0 {
                continue;
            }

            let color = css_color_to_peniko(shadow.color.clone());
            let brush = peniko::Brush::Solid(color);

            if r > 0.0 {
                let rr = RoundedRect::new(sx, sy, sw, sh, r);
                scene.fill(Fill::NonZero, Affine::IDENTITY, &brush, None, &rr);
            } else {
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &brush,
                    None,
                    &Rect::new(sx, sy, sx + sw, sy + sh),
                );
            }
        }
    }

    /// Draw a filled rectangle
    fn draw_rect(scene: &mut vello::Scene, style: &RenderStyle, x: f64, y: f64, w: f64, h: f64) {
        let brush = Self::make_brush(style);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &brush,
            None,
            &Rect::new(x, y, x + w, y + h),
        );
    }

    /// Draw a filled rounded rectangle
    fn draw_round_rect(
        scene: &mut vello::Scene,
        style: &RenderStyle,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
    ) {
        let brush = Self::make_brush(style);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &brush,
            None,
            &RoundedRect::new(x, y, x + w, y + h, radius),
        );
    }

    /// Draw a rectangle border
    fn draw_border(
        scene: &mut vello::Scene,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        width: f64,
        color: peniko::Color,
    ) {
        let stroke = Stroke::new(width);
        let rect = Rect::new(x, y, x + w, y + h);
        scene.stroke(&stroke, Affine::IDENTITY, color, None, &rect);
    }

    /// Draw an editable text input: value text, caret, selection highlight, and placeholder.
    #[allow(clippy::too_many_arguments)]
    fn draw_input(
        &mut self,
        scene: &mut vello::Scene,
        value: &str,
        font_size: f32,
        color: peniko::Color,
        font_family: Option<&str>,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        caret: usize,
        selection: Option<(usize, usize)>,
        focused: bool,
        caret_visible: bool,
        placeholder: Option<&str>,
    ) {
        let text_y = y + ((h - font_size as f64) / 2.0).max(0.0);
        let padding_x = 4.0;
        let text_x = x + padding_x;
        let text_width = (w - padding_x * 2.0).max(0.0);

        let display = if value.is_empty() {
            ""
        } else {
            value
        };

        if display.is_empty() {
            // Draw placeholder text in a dimmer color
            if let Some(ph) = placeholder {
                if !ph.is_empty() {
                    let placeholder_color = peniko::Color::from_rgba8(160, 160, 160, 255);
                    let layout = self.text.layout_text(
                        ph,
                        font_size,
                        font_family,
                        Some(text_width as f32),
                        None, None, None, None, None, None,
                    );
                    self.draw_layout_glyphs(scene, &layout, text_x, text_y, placeholder_color);
                }
            }
        } else {
            // Draw selection highlight first (behind text)
            if let Some((sel_start, sel_end)) = selection {
                if sel_start != sel_end {
                    let start = sel_start.min(sel_end);
                    let end = sel_start.max(sel_end);
                    let start_advance = self
                        .text
                        .measure_advance_before(display, font_size, font_family, start)
                        as f64;
                    let end_advance = self
                        .text
                        .measure_advance_before(display, font_size, font_family, end)
                        as f64;
                    let sel_x = text_x + start_advance;
                    let sel_w = end_advance - start_advance;
                    if sel_w > 0.0 {
                        let sel_color = peniko::Color::from_rgba8(51, 133, 255, 100);
                        let sel_brush = peniko::Brush::Solid(sel_color);
                        scene.fill(
                            Fill::NonZero,
                            Affine::IDENTITY,
                            &sel_brush,
                            None,
                            &Rect::new(sel_x, text_y, sel_x + sel_w, text_y + font_size as f64),
                        );
                    }
                }
            }

            // Draw the value text
            let layout = self.text.layout_text(
                display,
                font_size,
                font_family,
                Some(text_width as f32),
                None, None, None, None, None, None,
            );
            self.draw_layout_glyphs(scene, &layout, text_x, text_y, color);
        }

        // Draw the caret when focused and visible
        if focused && caret_visible {
            let caret_x = if display.is_empty() {
                text_x
            } else {
                text_x
                    + self.text
                        .measure_advance_before(display, font_size, font_family, caret)
                        as f64
            };
            let caret_color = color;
            let caret_brush = peniko::Brush::Solid(caret_color);
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &caret_brush,
                None,
                &Rect::new(
                    caret_x,
                    text_y,
                    caret_x + 1.5,
                    text_y + font_size as f64,
                ),
            );
        }
    }

    /// Draw glyph runs from a pre-built parley layout.
    fn draw_layout_glyphs(
        &mut self,
        scene: &mut vello::Scene,
        layout: &parley::Layout<()>,
        x: f64,
        y: f64,
        color: peniko::Color,
    ) {
        for line in layout.lines() {
            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                scene
                    .draw_glyphs(run.font())
                    .font_size(run.font_size())
                    .brush(color)
                    .transform(Affine::translate((x, y)))
                    .normalized_coords(run.normalized_coords())
                    .draw(
                        Fill::NonZero,
                        glyph_run.positioned_glyphs().map(|g| vello::Glyph {
                            id: g.id,
                            x: g.x,
                            y: g.y,
                        }),
                    );
            }
        }
    }

    /// Draw a checkmark inside a checkbox box.
    fn draw_checkmark(
        scene: &mut vello::Scene,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: peniko::Color,
    ) {
        let stroke = Stroke::new(2.0).with_caps(vello::kurbo::Cap::Round).with_join(vello::kurbo::Join::Round);
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        let s = w.min(h) * 0.3;
        // Checkmark path: down-left then up-right
        let mut path = vello::kurbo::BezPath::new();
        path.move_to((cx - s, cy));
        path.line_to((cx - s * 0.3, cy + s * 0.7));
        path.line_to((cx + s, cy - s * 0.6));
        scene.stroke(&stroke, Affine::IDENTITY, color, None, &path);
    }

    /// Draw a filled circle (radio dot) inside a radio button box.
    fn draw_radio_dot(
        scene: &mut vello::Scene,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: peniko::Color,
    ) {
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        let r = w.min(h) * 0.25;
        let brush = peniko::Brush::Solid(color);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &brush,
            None,
            &vello::kurbo::Circle::new((cx, cy), r),
        );
    }

    /// Create a peniko::Brush from a Background
    pub fn make_brush(style: &RenderStyle) -> peniko::Brush {
        match &style.background {
            Some(Background::Solid(color)) => (*color).into(),
            Some(Background::LinearGradient { start, end, stops }) => {
                let color_stops: Vec<peniko::ColorStop> = stops
                    .iter()
                    .map(|(offset, color)| peniko::ColorStop {
                        offset: *offset,
                        color: (*color).into(),
                    })
                    .collect();
                let gradient = peniko::Gradient::new_linear(
                    (start[0] as f64, start[1] as f64),
                    (end[0] as f64, end[1] as f64),
                )
                .with_stops(color_stops.as_slice());
                peniko::Brush::Gradient(gradient)
            }
            Some(Background::RadialGradient {
                center,
                radius,
                stops,
            }) => {
                let color_stops: Vec<peniko::ColorStop> = stops
                    .iter()
                    .map(|(offset, color)| peniko::ColorStop {
                        offset: *offset,
                        color: (*color).into(),
                    })
                    .collect();
                let gradient =
                    peniko::Gradient::new_radial((center[0] as f64, center[1] as f64), *radius)
                        .with_stops(color_stops.as_slice());
                peniko::Brush::Gradient(gradient)
            }
            None => palette::css::TRANSPARENT.into(),
        }
    }
}

impl Default for SceneBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Background, LayoutInfo};

    /// Number of encoded fills/strokes — a proxy for "something was drawn".
    fn path_count(scene: &vello::Scene) -> usize {
        scene.encoding().n_paths as usize
    }

    /// Number of positioned glyphs encoded into the scene.
    fn glyph_count(scene: &vello::Scene) -> usize {
        scene.encoding().resources.glyphs.len()
    }

    #[test]
    fn test_build_empty_scene() {
        let scene = RenderScene::new();
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        // Only the surface background fill.
        assert_eq!(path_count(&vello_scene), 1);
    }

    #[test]
    fn test_draw_rect() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(10.0, 20.0, 100.0, 50.0),
            palette::css::RED,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 2, "background + rect");
    }

    #[test]
    fn test_draw_rounded_rect() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::round_rect(
            1,
            LayoutInfo::new(10.0, 20.0, 100.0, 50.0),
            palette::css::BLUE,
            8.0,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 2);
    }

    #[test]
    fn test_draw_with_opacity() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
            palette::css::GREEN,
        );
        node.style.opacity = 0.5;
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(path_count(&vello_scene) >= 2);
    }

    #[test]
    fn test_solid_brush() {
        let style = RenderStyle {
            background: Some(Background::Solid(palette::css::RED)),
            ..Default::default()
        };
        let brush = SceneBuilder::make_brush(&style);
        assert!(matches!(brush, peniko::Brush::Solid(_)));
    }

    #[test]
    fn test_gradient_brush() {
        let style = RenderStyle {
            background: Some(Background::LinearGradient {
                start: [0.0, 0.0],
                end: [100.0, 0.0],
                stops: vec![(0.0, palette::css::RED), (1.0, palette::css::BLUE)],
            }),
            ..Default::default()
        };
        let brush = SceneBuilder::make_brush(&style);
        assert!(matches!(brush, peniko::Brush::Gradient(_)));
    }

    #[test]
    fn test_draw_border() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(
            1,
            LayoutInfo::new(10.0, 10.0, 100.0, 50.0),
            palette::css::WHITE,
        );
        node.style.border = Some(crate::scene::BorderStyle {
            width: 2.0,
            color: palette::css::BLACK,
        });
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 3, "background + fill + stroke");
    }

    #[test]
    fn test_skip_zero_size_node() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 0.0, 0.0),
            palette::css::RED,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 1, "only the background");
    }

    // ── Text rendering (M1) ─────────────────────────────────────

    #[test]
    fn test_text_node_encodes_glyphs() {
        // The old implementation drew a fixed 100px placeholder rect and dropped
        // the string entirely; nothing readable reached the screen.
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::text(
            1,
            LayoutInfo::new(10.0, 10.0, 400.0, 30.0),
            "Hello from uwebr!",
            24.0,
            palette::css::WHITE,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);

        if glyph_count(&vello_scene) == 0 {
            // No system fonts (headless CI): parley cannot shape anything.
            // Assert we at least did not emit a bogus placeholder rectangle.
            assert_eq!(path_count(&vello_scene), 1);
        } else {
            assert!(
                glyph_count(&vello_scene) >= 5,
                "expected glyphs for a 17-char string, got {}",
                glyph_count(&vello_scene)
            );
        }
    }

    #[test]
    fn test_empty_text_draws_nothing() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::text(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 20.0),
            "   ",
            16.0,
            palette::css::WHITE,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(glyph_count(&vello_scene), 0);
        assert_eq!(path_count(&vello_scene), 1, "no placeholder rect");
    }

    #[test]
    fn test_text_does_not_emit_placeholder_rect() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::text(
            1,
            LayoutInfo::new(0.0, 0.0, 300.0, 20.0),
            "abc",
            16.0,
            palette::css::RED,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        // Glyphs are encoded as glyph runs, not as extra filled paths.
        assert_eq!(path_count(&vello_scene), 1);
    }

    #[test]
    fn test_background_and_text_both_encoded() {
        let mut scene = RenderScene::new();
        let mut container = RenderNode::container(1, LayoutInfo::new(0.0, 0.0, 800.0, 600.0));
        container.style.background = Some(Background::Solid(peniko::Color::from_rgb8(
            0x1a, 0x1a, 0x2e,
        )));
        scene.add_node(container);
        scene.add_node(RenderNode::text(
            2,
            LayoutInfo::new(10.0, 10.0, 400.0, 30.0),
            "Hello",
            32.0,
            peniko::Color::from_rgb8(0xe0, 0xe0, 0xe0),
        ));

        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(
            path_count(&vello_scene),
            2,
            "surface background + container background"
        );
    }

    #[test]
    fn test_container_without_background_draws_nothing() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::container(
            1,
            LayoutInfo::new(0.0, 0.0, 10.0, 10.0),
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 1);
    }

    #[test]
    fn test_overflow_hidden_pushes_clip() {
        let mut scene = RenderScene::new();
        let mut node =
            RenderNode::rect(1, LayoutInfo::new(0.0, 0.0, 50.0, 50.0), palette::css::RED);
        node.style.overflow_hidden = true;
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            vello_scene.encoding().n_clips > 0,
            "overflow:hidden must encode a clip layer"
        );
    }

    #[test]
    fn test_container_background_radius_uses_rounded_rect() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::container(1, LayoutInfo::new(0.0, 0.0, 40.0, 40.0));
        node.style.background = Some(Background::Solid(palette::css::RED));
        node.style.border_radius = 6.0;
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 2);
    }

    #[test]
    fn test_reused_builder_produces_same_result() {
        // The builder is long-lived in the render loop; repeated builds must not
        // accumulate state.
        let mut builder = SceneBuilder::new();
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 10.0, 10.0),
            palette::css::RED,
        ));
        let first = path_count(&builder.build(&scene, 800, 600));
        let second = path_count(&builder.build(&scene, 800, 600));
        assert_eq!(first, second);
    }

    // ── Image rendering (FAZ 11) ────────────────────────────────

    /// A 2x2 red RGBA PNG, encoded in memory for the decode path.
    fn tiny_png() -> Vec<u8> {
        use image::{ImageEncoder, ImageFormat};
        let mut buf = std::io::Cursor::new(Vec::new());
        let pixels: [u8; 16] = [
            255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        ];
        image::codecs::png::PngEncoder::new(&mut buf)
            .write_image(&pixels, 2, 2, image::ExtendedColorType::Rgba8)
            .unwrap();
        let _ = ImageFormat::Png;
        buf.into_inner()
    }

    #[test]
    fn test_draw_valid_image_encodes_something() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::image(
            1,
            LayoutInfo::new(0.0, 0.0, 64.0, 64.0),
            tiny_png(),
            2,
            2,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        // Surface background + the image fill.
        assert!(
            path_count(&vello_scene) >= 2,
            "image should encode a fill, got {}",
            path_count(&vello_scene)
        );
    }

    #[test]
    fn test_draw_invalid_image_is_skipped() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::image(
            1,
            LayoutInfo::new(0.0, 0.0, 64.0, 64.0),
            vec![0xde, 0xad, 0xbe, 0xef],
            2,
            2,
        ));
        // Must not panic; invalid data draws nothing beyond the background.
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 1, "only the surface background");
    }

    // ── Text overflow: ellipsis (FAZ 11) ────────────────────────

    #[test]
    fn test_truncate_short_text_unchanged() {
        let mut builder = SceneBuilder::new();
        // A wide box relative to the string: no truncation needed.
        let out = builder.truncate_with_ellipsis("Hi", 16.0, None, 10_000.0);
        assert_eq!(out, "Hi");
    }

    #[test]
    fn test_truncate_long_text_gets_ellipsis() {
        let mut builder = SceneBuilder::new();
        let long = "This is a very long string that will not fit in a tiny box";
        let out = builder.truncate_with_ellipsis(long, 16.0, None, 40.0);
        // With a real font the string is shortened and ends with the ellipsis.
        // Without system fonts advances are zero, so the string fits unchanged;
        // in that case the fallback path returns the original text.
        if out != long {
            assert!(
                out.ends_with('…'),
                "truncated text should end with ellipsis"
            );
            assert!(out.chars().count() < long.chars().count());
        }
    }

    #[test]
    fn test_ellipsis_text_node_draws_without_panic() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::text(
            1,
            LayoutInfo::new(0.0, 0.0, 30.0, 20.0),
            "A long piece of text that overflows",
            16.0,
            palette::css::WHITE,
        );
        node.style.text_overflow = TextOverflow::Ellipsis;
        scene.add_node(node);
        // Should not panic during measurement/truncation.
        let _ = SceneBuilder::build_scene(&scene, 800, 600);
    }

    // ── Scene builder edge-case tests ───────────────────────────

    #[test]
    fn render_multistop_gradient_background() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 200.0, 200.0),
            palette::css::RED,
        );
        node.style.background = Some(Background::LinearGradient {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
            stops: vec![
                (0.0, palette::css::RED),
                (0.33, palette::css::GREEN),
                (0.66, palette::css::BLUE),
                (1.0, palette::css::YELLOW),
            ],
        });
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 2,
            "gradient should encode a path"
        );
    }

    #[test]
    fn render_round_rect_different_radius() {
        let mut scene = RenderScene::new();
        let node = RenderNode::round_rect(
            1,
            LayoutInfo::new(10.0, 10.0, 100.0, 60.0),
            palette::css::BLUE,
            16.0,
        );
        let radius = node.style.border_radius;
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 2);
        assert_eq!(radius, 16.0);
    }

    #[test]
    fn render_zero_opacity_element() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
            palette::css::RED,
        );
        node.style.opacity = 0.0;
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 1,
            "zero opacity still draws (opacity layer pushed)"
        );
    }

    #[test]
    fn render_empty_text_node() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::text(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 20.0),
            "",
            16.0,
            palette::css::WHITE,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(
            glyph_count(&vello_scene),
            0,
            "empty text should not emit glyphs"
        );
    }

    #[test]
    fn render_deeply_nested_clip_layers() {
        let mut scene = RenderScene::new();
        for i in 0..5 {
            let mut node = RenderNode::rect(
                i,
                LayoutInfo::new(10.0 * i as f64 as f32, 10.0 * i as f64 as f32, 200.0, 200.0),
                palette::css::GREEN,
            );
            node.style.overflow_hidden = true;
            scene.add_node(node);
        }
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            vello_scene.encoding().n_clips >= 5,
            "5 nested clip layers should produce at least 5 clips"
        );
    }

    #[test]
    fn render_multiple_gradient_backgrounds() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
            palette::css::RED,
        ));
        let mut node2 = RenderNode::rect(
            2,
            LayoutInfo::new(50.0, 50.0, 100.0, 100.0),
            palette::css::BLUE,
        );
        node2.style.background = Some(Background::LinearGradient {
            start: [0.0, 0.0],
            end: [0.0, 1.0],
            stops: vec![(0.0, palette::css::RED), (1.0, palette::css::BLUE)],
        });
        scene.add_node(node2);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 3,
            "two nodes with backgrounds + surface bg"
        );
    }

    #[test]
    fn render_radial_gradient_brush() {
        let style = RenderStyle {
            background: Some(Background::RadialGradient {
                center: [0.5, 0.5],
                radius: 0.5,
                stops: vec![
                    (0.0, palette::css::RED),
                    (0.5, palette::css::GREEN),
                    (1.0, palette::css::BLUE),
                ],
            }),
            ..Default::default()
        };
        let brush = SceneBuilder::make_brush(&style);
        assert!(matches!(brush, peniko::Brush::Gradient(_)));
    }

    #[test]
    fn render_negative_position_element() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(-50.0, -50.0, 100.0, 100.0),
            palette::css::RED,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 2,
            "negative position should still draw"
        );
    }

    #[test]
    fn render_very_large_dimensions() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 10000.0, 10000.0),
            palette::css::RED,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 2,
            "very large rect should still draw"
        );
    }

    #[test]
    fn render_container_border_radius_and_background() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::container(1, LayoutInfo::new(0.0, 0.0, 200.0, 100.0));
        node.style.background = Some(Background::Solid(palette::css::BLUE));
        node.style.border_radius = 20.0;
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(
            path_count(&vello_scene),
            2,
            "rounded container bg + surface bg"
        );
    }

    #[test]
    fn render_multiple_borders() {
        let mut scene = RenderScene::new();
        let mut node1 = RenderNode::rect(
            1,
            LayoutInfo::new(10.0, 10.0, 100.0, 50.0),
            palette::css::RED,
        );
        node1.style.border = Some(crate::scene::BorderStyle {
            width: 2.0,
            color: palette::css::BLACK,
        });
        let mut node2 = RenderNode::rect(
            2,
            LayoutInfo::new(120.0, 10.0, 100.0, 50.0),
            palette::css::BLUE,
        );
        node2.style.border = Some(crate::scene::BorderStyle {
            width: 3.0,
            color: palette::css::WHITE,
        });
        scene.add_node(node1);
        scene.add_node(node2);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 5,
            "2 fills + 2 strokes + surface bg"
        );
    }

    #[test]
    fn render_opacity_and_overflow_hidden_combo() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
            palette::css::GREEN,
        );
        node.style.opacity = 0.5;
        node.style.overflow_hidden = true;
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            vello_scene.encoding().n_clips > 0,
            "overflow_hidden should push clip"
        );
        assert!(path_count(&vello_scene) >= 2);
    }

    #[test]
    fn render_gradient_many_stops() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 400.0, 400.0),
            palette::css::RED,
        );
        node.style.background = Some(Background::LinearGradient {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
            stops: vec![
                (0.0, palette::css::RED),
                (0.1, palette::css::ORANGE),
                (0.2, palette::css::YELLOW),
                (0.3, palette::css::GREEN),
                (0.4, palette::css::CYAN),
                (0.5, palette::css::BLUE),
                (0.6, palette::css::MAGENTA),
                (0.7, palette::css::RED),
                (0.8, palette::css::GREEN),
                (0.9, palette::css::BLUE),
                (1.0, palette::css::WHITE),
            ],
        });
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 2,
            "11-stop gradient should encode without panic"
        );
    }

    #[test]
    fn render_container_with_border_no_background() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::container(1, LayoutInfo::new(10.0, 10.0, 200.0, 100.0));
        node.style.border = Some(crate::scene::BorderStyle {
            width: 1.0,
            color: palette::css::RED,
        });
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(
            path_count(&vello_scene),
            2,
            "container without bg but with border = stroke + surface bg"
        );
    }

    #[test]
    fn render_zero_size_negative_position_skipped() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(-10.0, -10.0, 0.0, 0.0),
            palette::css::RED,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(
            path_count(&vello_scene),
            1,
            "zero-size node should be skipped"
        );
    }

    #[test]
    fn render_make_brush_transparent_background() {
        let style = RenderStyle {
            background: None,
            ..Default::default()
        };
        let brush = SceneBuilder::make_brush(&style);
        assert!(matches!(brush, peniko::Brush::Solid(_)));
    }

    // ── Quality tests (test_q_*) ────────────────────────────────

    #[test]
    fn test_q_scene_builder_gradient_background() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 200.0, 200.0),
            palette::css::RED,
        );
        node.style.background = Some(Background::LinearGradient {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
            stops: vec![(0.0, palette::css::RED), (1.0, palette::css::BLUE)],
        });
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 2,
            "gradient must encode at least 1 path + surface bg"
        );
    }

    #[test]
    fn test_q_stress_many_gradients() {
        let mut scene = RenderScene::new();
        for i in 0..100 {
            let mut node = RenderNode::rect(
                i,
                LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
                palette::css::RED,
            );
            node.style.background = Some(Background::LinearGradient {
                start: [0.0, 0.0],
                end: [1.0, 0.0],
                stops: vec![(0.0, palette::css::RED), (1.0, palette::css::BLUE)],
            });
            scene.add_node(node);
        }
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 101,
            "100 gradients + surface bg must encode, got {}",
            path_count(&vello_scene)
        );
    }

    #[test]
    fn test_q_stress_scene_builder_500_nodes() {
        let mut scene = RenderScene::new();
        for i in 0..500 {
            scene.add_node(RenderNode::rect(
                i,
                LayoutInfo::new(0.0, 0.0, 10.0, 10.0),
                palette::css::RED,
            ));
        }
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 501,
            "500 rects + surface bg, got {}",
            path_count(&vello_scene)
        );
    }

    // ── z-index sort tests ───────────────────────────────────

    #[test]
    fn test_z_index_sorted_paint_order() {
        let mut scene = RenderScene::new();
        // Add nodes in reverse z-index order — the builder must sort them.
        for i in (0..5u64).rev() {
            let mut node = RenderNode::rect(
                i,
                LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
                palette::css::RED,
            );
            node.style.z_index = i as i32;
            scene.add_node(node);
        }
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        // All 5 rects + surface bg should render.
        assert!(
            path_count(&vello_scene) >= 6,
            "5 z-indexed rects + bg must encode, got {}",
            path_count(&vello_scene)
        );
    }

    #[test]
    fn test_z_index_mixed_values() {
        let mut scene = RenderScene::new();
        let mut a = RenderNode::rect(0, LayoutInfo::new(0.0, 0.0, 50.0, 50.0), palette::css::RED);
        a.style.z_index = -1;
        let mut b = RenderNode::rect(
            1,
            LayoutInfo::new(10.0, 0.0, 50.0, 50.0),
            palette::css::GREEN,
        );
        b.style.z_index = 10;
        let mut c = RenderNode::rect(
            2,
            LayoutInfo::new(20.0, 0.0, 50.0, 50.0),
            palette::css::BLUE,
        );
        c.style.z_index = 0;
        scene.add_node(a);
        scene.add_node(b);
        scene.add_node(c);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 4,
            "3 z-indexed rects + bg must encode, got {}",
            path_count(&vello_scene)
        );
    }

    // ── Transform tests ──────────────────────────────────────

    #[test]
    fn test_transform_to_affine_identity() {
        use uwebr_css::codegen::TransformProps;
        let props = TransformProps::default();
        let aff = SceneBuilder::transform_to_affine(&props, 0.0, 0.0);
        assert_eq!(aff, Affine::IDENTITY);
    }

    #[test]
    fn test_transform_to_affine_translate() {
        use uwebr_css::codegen::TransformProps;
        let props = TransformProps {
            translate_x: Some(10.0),
            translate_y: Some(20.0),
            ..Default::default()
        };
        let aff = SceneBuilder::transform_to_affine(&props, 0.0, 0.0);
        assert_eq!(aff, Affine::translate((10.0, 20.0)));
    }

    #[test]
    fn test_transform_to_affine_rotate_90() {
        use uwebr_css::codegen::TransformProps;
        let props = TransformProps {
            rotate: Some(90.0),
            ..Default::default()
        };
        let aff = SceneBuilder::transform_to_affine(&props, 100.0, 50.0);
        // Rotation should not be identity.
        assert_ne!(aff, Affine::IDENTITY);
    }

    #[test]
    fn test_transform_to_affine_scale() {
        use uwebr_css::codegen::TransformProps;
        let props = TransformProps {
            scale_x: Some(2.0),
            scale_y: Some(3.0),
            ..Default::default()
        };
        let aff = SceneBuilder::transform_to_affine(&props, 0.0, 0.0);
        assert_ne!(aff, Affine::IDENTITY);
    }

    #[test]
    fn test_node_with_transform_renders() {
        use uwebr_css::codegen::TransformProps;
        let mut node = RenderNode::rect(
            0,
            LayoutInfo::new(50.0, 50.0, 200.0, 100.0),
            palette::css::BLUE,
        );
        node.transform = TransformProps {
            translate_x: Some(10.0),
            rotate: Some(45.0),
            ..Default::default()
        };
        let mut scene = RenderScene::new();
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            path_count(&vello_scene) >= 2,
            "transformed rect + bg must encode, got {}",
            path_count(&vello_scene)
        );
    }
}
