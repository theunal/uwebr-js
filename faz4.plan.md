# FAZ 4: `uwebr-render` GPU Pipeline

## Genel Mimari

Element → Layout → Scene → GPU

```text
Element (uwebr-core)          CSS Rules (uwebr-css)
     │                              │
     ▼                              ▼
┌──────────────────────────────────────────────┐
│  LayoutEngine (taffy 0.14)                   │
│  Element → TaffyTree → compute_layout()      │
│  → Vec<PositionedNode>                       │
└──────────────────┬───────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────┐
│  SceneBuilder                                │
│  PositionedNode + css::Color → vello::Scene  │
│  fill(), stroke(), draw_glyphs(), layers     │
└──────────────────┬───────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────┐
│  Renderer (wgpu + vello)                     │
│  Scene → render_to_texture → surface blit    │
└──────────────────────────────────────────────┘
```

## Modül 1: `scene.rs` - Zenginleştirilmiş Scene Graph

**Mevcut:** Basit `SceneNode { id, kind, x, y, width, height }`

**Yeni tasarıma göre:**

```rust
// scene.rs — tamamen yeniden yazılacak

pub struct RenderScene {
    nodes: Vec<RenderNode>,
}

pub struct RenderNode {
    pub id: u64,
    pub kind: RenderNodeKind,
    pub layout: LayoutInfo,       // taffy'den gelen pozisyon + boyut
    pub style: RenderStyle,       // görsel stil
}

pub struct LayoutInfo {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub struct RenderStyle {
    pub background: Option<Background>,
    pub border: Option<BorderStyle>,
    pub border_radius: f32,
    pub opacity: f32,
    pub overflow_hidden: bool,
}

pub enum Background {
    Solid(peniko::Color),
    LinearGradient { start: [f32; 2], end: [f32; 2], stops: Vec<(f32, peniko::Color)> },
    RadialGradient { center: [f32; 2], radius: f32, stops: Vec<(f32, peniko::Color)> },
}

pub struct BorderStyle {
    pub width: f32,
    pub color: peniko::Color,
}

pub enum RenderNodeKind {
    Rect,
    RoundRect { radius: f32 },
    Text { content: String, font_size: f32, color: peniko::Color },
    Image { data: Vec<u8>, width: u32, height: u32 },
    Container,  // sadece layout için, çizim yok (sadece clip varsa)
}
```

### Testler
Testler:
- test_render_scene_add_node — node ekleme
- test_render_style_defaults — default stil değerleri
- test_background_solid — solid color arka plan
- test_background_gradient — linear gradient

## Modül 2: `layout.rs` - Element → TaffyTree → Positioned Nodes

**Mevcut:** Boş `LayoutEngine`

**Yeni:**

```rust
use taffy::prelude::*;
use uwebr_core::component::{Element, NodeType};

pub struct LayoutEngine {
    taffy: TaffyTree<()>,
}

pub struct PositionedNode {
    pub taffy_node: taffy::NodeId,
    pub element: Element,        // gốc element referansı
    pub depth: usize,            // tree depth (z-order için)
}

impl LayoutEngine {
    pub fn new() -> Self { ... }

    /// Element tree'sini TaffyTree'ye çevir
    pub fn build_tree(&mut self, root: &Element) -> Result<taffy::NodeId> { ... }

    /// TaffyTree'de layout hesapla
    pub fn compute(&mut self, root: taffy::NodeId, width: f32, height: f32) -> Result<()> { ... }

    /// Hesaplanmış layout bilgilerini çıkar
    pub fn get_layout_info(&self, node: taffy::NodeId) -> Result<LayoutInfo> { ... }

    /// Tüm tree'yi positioned node listesine çevir
    pub fn collect_positioned_nodes(&self, root: taffy::NodeId) -> Vec<PositionedNode> { ... }

    pub fn reset(&mut self) { ... }
}
```

### Element → Taffy Style Dönüşümü
Element → Taffy Style dönüşümü:
- NodeType::Element("div") → Display::Flex (default)
- NodeType::Text(_) → Display::Inline + measure function
- CSS prop'ları → uwebr_css::codegen::convert_to_taffy_styles() kullan
- Inline style attribute → doğrudan Style'a uygula

### Testler
- test_build_simple_tree — tek element
- test_build_nested_tree — parent-child
- test_compute_layout — layout hesaplama
- test_collect_positioned_nodes — pozisyon çıkarma
- test_text_node_measurement — text node'un intrinsic boyutu
- test_css_class_application — CSS class → taffy style

## Modül 3: `scene_builder.rs` - Positioned Nodes → Vello Scene

**Yeni dosya:**

```rust
use vello::Scene;
use vello::kurbo::{Affine, Rect, RoundedRect, Line, Circle};
use vello::peniko::{Fill, Stroke, color::palette};

pub struct SceneBuilder;

impl SceneBuilder {
    /// PositionedNode listesinden vello Scene oluştur
    pub fn build_scene(
        nodes: &[PositionedNode],
        width: u32,
        height: u32,
    ) -> vello::Scene { ... }

    /// Tek bir node'u vello draw call'ına çevir
    fn draw_node(scene: &mut Scene, node: &PositionedNode) { ... }

    /// Background brush oluştur
    fn make_brush(bg: &Background) -> peniko::Brush { ... }
}
}
```

### Draw Call Eşleştirmeleri

| RenderNodeKind | Vello draw call |
|---|---|
| `Rect` | `scene.fill(Fill::NonZero, transform, brush, None, &Rect::new(...))` |
| `RoundRect { radius }` | `scene.fill(Fill::NonZero, transform, brush, None, &RoundedRect::new(...))` |
| `Text { .. }` | `scene.draw_glyphs(&font).transform(...).font_size(...).brush(...).draw(...)` |
| `Container` | `scene.push_clip_layer(...)` / `scene.pop_layer()` (`overflow:hidden` varsa) |

### Gradient Dönüşümü

| Background | Vello |
|---|---|
| `Solid(color)` | `peniko::Color::from_rgba8(r, g, b, a)` |
| `LinearGradient { start, end, stops }` | `Gradient::new_linear(start, end).with_stops(stops)` |
| `RadialGradient { center, radius, stops }` | `Gradient::new_radial(center, radius).with_stops(stops)` |

- **Opacity:** `scene.push_layer(Fill::NonZero, Compose::SrcOver, opacity, transform, &bounds)`
- **Border-radius:** `RoundedRect::new(x, y, x+w, y+h, radius)`

### Testler
- test_build_empty_scene — boş scene
- test_draw_rect — rectangle çizimi
- test_draw_rounded_rect — rounded rect
- test_draw_with_opacity — opacity layer
- test_solid_brush — solid color brush
- test_gradient_brush — gradient brush

## Modül 4: `renderer.rs` - Gerçek GPU Pipeline

**Mevcut:** Boş `Renderer { width, height }`

**Yeni:**

```rust
use wgpu;
use vello::{Renderer as VelloRenderer, RendererOptions, RenderParams, Scene};
use vello::peniko::color::palette;

pub struct Renderer {
    // wgpu state
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    // vello state
    vello_renderer: VelloRenderer,

    // render state
    width: u32,
    height: u32,
    scene: RenderScene,
    needs_redraw: bool,
}

impl Renderer {
    /// Yeni renderer oluştur (wgpu device + vello)
    pub async fn new(window: &winit::window::Window) -> Result<Self> { ... }

    /// Pencere boyutu değiştiğinde
    pub fn resize(&mut self, width: u32, height: u32) { ... }

    /// Render scene'i GPU'ya çiz
    pub fn render(&mut self) -> Result<()> { ... }

    /// Scene'i güncelle (yeni positioned nodes ile)
    pub fn update_scene(&mut self, scene: RenderScene) { ... }

    /// Tek frame render et (scene builder → vello scene → GPU)
    pub fn render_frame(&mut self) -> Result<()> { ... }
}
```

### GPU Pipeline Akışı
1. scene.reset() — önceki frame'i temizle
2. SceneBuilder::build_scene(nodes, w, h) → vello Scene
3. Intermediate texture oluştur (Rgba8Unorm, STORAGE_BINDING)
4. vello_renderer.render_to_texture(device, queue, &scene, &texture_view, &params)
5. Texture'ı surface'a blit

### Testler (GPU Gerektirmeyen Unit Testler)
- test_renderer_creation_with_mock — renderer struct oluşumu
- test_resize_updates_dimensions — boyut güncelleme
- test_scene_update — scene güncelleme

## Modül 5: `text.rs` - Parley + Vello Text Rendering

**Yeni dosya:**

```rust
use parley::{FontContext, LayoutContext, Layout};
use vello::kurbo::Affine;
use vello::peniko::{self, Fill, color::palette};

pub struct TextRenderer {
    font_context: FontContext,
    layout_context: LayoutContext<()>,
}

pub struct TextRun {
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub color: peniko::Color,
    pub content: String,
}

impl TextRenderer {
    pub fn new() -> Self { ... }

    /// Text'i layout et
    pub fn layout_text(&mut self, content: &str, font_size: f32, max_width: f32) -> Layout<()> { ... }

    /// Layout edilmiş text'i vello Scene'e çiz
    pub fn draw_text(
        &self,
        scene: &mut vello::Scene,
        layout: &Layout<()>,
        x: f64,
        y: f64,
        color: peniko::Color,
    ) { ... }
}
```

### Parley Workflow
1. FontContext::new() — system font'ları yükle
2. LayoutContext::new() — layout context oluştur
3. ranged_builder(&font_context, text, font_size) → builder
4. builder.build() → Layout<()>
5. Layout'dan glyph'ları çıkar → scene.draw_glyphs(font_data).draw(glyphs)

### Testler
- test_text_renderer_creation — text renderer oluşturma
- test_layout_text_returns_layout — text layout
- test_draw_text_into_scene — scene'e text çizme

## Modül 6: `color.rs` - CSS Color → `peniko::Color` Dönüşümü

**Yeni dosya:**

```rust
use uwebr_css::ast::Color as CssColor;
use vello::peniko;

impl From<CssColor> for peniko::Color {
    fn from(c: CssColor) -> Self {
        peniko::Color::from_rgba8(c.r, c.g, c.b, (c.a * 255.0) as u8)
    }
}

/// Parse CSS color string ("#ff0000", "red", "rgb(255,0,0)") to peniko::Color
pub fn parse_color_to_peniko(color_str: &str) -> Option<peniko::Color> { ... }
```

### Testler
- test_css_color_to_peniko — CssColor → peniko::Color
- test_parse_hex_color — "#ff0000" parse
- test_parse_named_color — "red", "blue" parse

## Dosya Yapısı (Yeni/Değişen)

```text
crates/uwebr-render/src/
├── lib.rs              # mod tanımları + re-export'lar
├── renderer.rs         # GPU pipeline (wgpu + vello)
├── scene.rs            # RenderScene, RenderNode, RenderStyle, Background
├── scene_builder.rs    # PositionedNode → vello Scene dönüşümü (YENİ)
├── layout.rs           # Element → TaffyTree → PositionedNode
├── text.rs             # Parley + vello text rendering (YENİ)
└── color.rs            # CSS Color → peniko::Color (YENİ)
```

## Uygulama Sırası

| # | Adım | Dosya | Tahmini test |
|---:|---|---|---:|
| 1 | `color.rs` yaz - CSS Color → `peniko::Color` | `color.rs` | 3 |
| 2 | `scene.rs` yeniden yaz - `RenderScene`, `RenderNode`, `RenderStyle` | `scene.rs` | 4 |
| 3 | `text.rs` yaz - Parley + vello text | `text.rs` | 3 |
| 4 | `layout.rs` yeniden yaz - TaffyTree integration | `layout.rs` | 6 |
| 5 | `scene_builder.rs` yaz - PositionedNode → vello Scene | `scene_builder.rs` | 6 |
| 6 | `renderer.rs` yeniden yaz - gerçek GPU pipeline | `renderer.rs` | 3 |
| 7 | `lib.rs` güncelle - mod tanımları + export | `lib.rs` | - |
| 8 | `Cargo.toml` güncelle - parley, kurbo, peniko ekle | `Cargo.toml` | - |
| 9 | `cargo test -p uwebr-render` - tüm testler | - | 25 |
| 10 | `PLAN.md` güncelle | `PLAN.md` | - |

## Dependencies (`Cargo.toml` Değişiklikleri)

```toml
[dependencies]
anyhow.workspace = true
thiserror.workspace = true
winit.workspace = true
wgpu.workspace = true
vello.workspace = true
taffy.workspace = true
parley.workspace = true      # YENİ — text layout
kurbo.workspace = true        # YENİ — 2D geometry (vello re-export eder ama bağımsız kullan)
uwebr-core.workspace = true   # YENİ — Element, NodeType
uwebr-css.workspace = true    # YENİ — CssColor, convert_to_taffy_styles
```

## Test Hedefi

| Modül | Test |
|---|---:|
| `color.rs` | 3 |
| `scene.rs` | 4 |
| `text.rs` | 3 |
| `layout.rs` | 6 |
| `scene_builder.rs` | 6 |
| `renderer.rs` | 3 |
| **Toplam** | **25** |

**Workspace toplamı:** 141 (mevcut) + 25 (yeni) = **166 test**