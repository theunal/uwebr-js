# FAZ 4: `uwebr-render` GPU Pipeline — TAMAMLANDI

> Durum: ✅ tamamlandı. Bu belge özgün planı ve **uygulamanın plandan saptığı yerleri** kaydeder.
> Son güncelleme: 28 Ağustos 2026 (FAZ 8 sonrası).

## Sonuç Özeti

| Modül | Plan | Gerçekleşen |
|-------|------|-------------|
| `color.rs` | 3 test | ✅ 6 test, `From` impl yerine serbest fonksiyon |
| `scene.rs` | 4 test | ✅ 8 test, `Text` kolu `font_family` de taşıyor |
| `text.rs` | 3 test | ✅ 10 test, fontsuz ortam için tahmin yolu eklendi |
| `layout.rs` | 6 test | ✅ 15 test, `TaffyTree<NodeContext>` + measure function |
| `scene_builder.rs` | 6 test | ✅ 16 test, gerçek `draw_glyphs` |
| `renderer.rs` | 3 test | ✅ 6 test, **GPU state taşımıyor** (bkz. Sapma 1) |
| `stylebook.rs` | plansız | ✅ 20 test (FAZ 4.5 + FAZ 8 cascade) |
| `paint.rs` | plansız | ✅ 12 test (FAZ 8) |
| **Toplam** | 25 | **93 test** |

`cargo test -p uwebr-render` → 93 test geçiyor.

## Planlanan Mimari

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

Son iki kutu gerçekleşti, ancak alt kutu `uwebr-app`'e taşındı.

---

## Plandan Sapmalar

### Sapma 1 — GPU state `uwebr-render`'da değil, `uwebr-app`'te

Plan `renderer.rs`'in `wgpu::Device`, `Queue`, `Surface` ve `vello::Renderer` tutmasını öngörüyordu. Uygulamada bunlar `uwebr-app::GpuContext`'te:

```rust
// crates/uwebr-render/src/renderer.rs — gerçekleşen
pub struct Renderer {
    width: u32,
    height: u32,
    scene: RenderScene,
    needs_redraw: bool,
    builder: SceneBuilder,   // sahne birleştirici; GPU yok
}
```

**Neden:** GPU cihazı bir pencereye bağlıdır (`Surface<'static>` bir `winit::Window` gerektirir). `uwebr-render` pencere bilmemeli; aksi halde `wgpu` ve `winit`'e bağımlı olur ve headless test edilemez. FAZ 8'de `uwebr-render`'ın `Cargo.toml`'undan `wgpu` ve `winit` kaldırıldı — bu ayrıca vello 0.10'un beklediği wgpu 29 ile çakışan ikinci bir wgpu 30 kopyasını da ortadan kaldırdı.

**Sonuç:** `renderer.rs` adı yanıltıcı; işi "sahne birleştirici". Doküman düzeltildi, isim korunuyor (public API).

### Sapma 2 — `TaffyTree<()>` yerine `TaffyTree<NodeContext>`

Plan `TaffyTree<()>` diyordu; text node'lar `new_leaf` ile ölçüsüz eklenirdi. Bu FAZ 8'e kadar böyle kaldı ve **metnin hiç görünmemesinin ana nedeniydi**: column flex içinde 0 yükseklik hesaplanıyor, `pipeline.rs` de 0 boyutlu node'ları filtreliyordu.

Gerçekleşen:

```rust
pub enum NodeContext {
    Text { content: String, font_size: f32, font_family: Option<String> },
}

pub struct LayoutEngine {
    taffy: TaffyTree<NodeContext>,
    text: TextRenderer,      // parley context'i yeniden kullanılır
}
```

`compute_layout` yerine `compute_layout_with_measure` çağrılıyor; measure closure `TextRenderer::measure`'a düşüyor.

### Sapma 3 — `PositionedNode` daha fazla veri taşıyor

Plan:

```rust
pub struct PositionedNode {
    pub taffy_node: taffy::NodeId,
    pub element: Element,
    pub depth: usize,
}
```

Gerçekleşen — `layout` (mutlak koordinat) ve `paint` eklendi:

```rust
pub struct PositionedNode {
    pub taffy_node: taffy::NodeId,
    pub element: Element,
    pub layout: LayoutInfo,      // MUTLAK x/y (taffy parent-relative verir)
    pub depth: usize,
    pub paint: ResolvedPaint,    // FAZ 8: renk/font, kalıtımla çözülmüş
}
```

**Neden mutlak koordinat:** taffy çocuk konumlarını ebeveyne göre raporlar, sahne ise tek düz koordinat uzayında çizer. `collect_recursive` ebeveyn offset'ini toplayarak iniyor; aksi halde iç içe içerik yanlış yere çizilir ve tıklama hedefleri kayar.

### Sapma 4 — `SceneBuilder` artık birim struct değil

Plan `pub struct SceneBuilder;` (state'siz) diyordu. Metin çizimi bir parley `FontContext` gerektiriyor ve onu her frame kurmak sistem font koleksiyonunu yeniden taramak demek:

```rust
pub struct SceneBuilder {
    text: TextRenderer,
}

impl SceneBuilder {
    pub fn build(&mut self, scene: &RenderScene, width: u32, height: u32) -> vello::Scene;
    /// Kendi TextRenderer'ını kuran kolaylık sarmalayıcısı (testler için).
    pub fn build_scene(scene: &RenderScene, width: u32, height: u32) -> vello::Scene;
}
```

`RenderPipeline` ve `Renderer` uzun ömürlü bir `SceneBuilder` tutuyor.

### Sapma 5 — `color.rs`'te `From` impl yok

Plan `impl From<CssColor> for peniko::Color` öneriyordu. Her iki tip de dış crate'lerden geldiği için orphan rule bunu engelliyor. Serbest fonksiyon kullanıldı:

```rust
pub fn css_color_to_peniko(c: CssColor) -> peniko::Color;
pub fn parse_color_to_peniko(color_str: &str) -> Option<peniko::Color>;
```

FAZ 8'e kadar `css_color_to_peniko` ölü koddu (yalnız kendi testinden çağrılıyordu); artık `ResolvedPaint::apply_css` içinden kullanılıyor.

### Sapma 6 — `text.rs`'te `draw_text` yok, `measure` var

Plan `TextRenderer::draw_text(scene, layout, x, y, color)` öngörüyordu. Uygulamada çizim `scene_builder.rs::draw_text` içinde; `text.rs` yalnız layout ve ölçüm sağlıyor:

```rust
pub fn layout_text(&mut self, content: &str, font_size: f32,
                   font_family: Option<&str>, max_advance: Option<f32>) -> Layout<()>;
pub fn measure(&mut self, content: &str, font_size: f32,
               font_family: Option<&str>, max_advance: Option<f32>) -> (f32, f32);
```

**Neden ayrıldı:** ölçüm layout aşamasında (taffy measure closure), çizim sahne aşamasında gerekiyor. Aynı `TextRenderer` iki farklı çağrı noktasından kullanılıyor.

Ek olarak `estimate_text_size` eklendi: parley sistem fontu bulamazsa 0 boyut döndürür ve metin node'u sahneden düşer. Tahmin yolu bunu engelliyor.

### Sapma 7 — Ara texture kararı doğrulandı

Plandaki "Intermediate texture oluştur (Rgba8Unorm, STORAGE_BINDING)" adımı FAZ 4'te uygulanmadı; `context.rs` doğrudan surface view'a çiziyordu. FAZ 8'de gerçek GPU'da çalıştırılınca panikledi:

```
wgpu error: Validation Error
  In Device::create_bind_group
    Storage texture binding 5 expects format = Rgba8Unorm,
    but given a view with format = Bgra8UnormSrgb
```

Planın önerdiği yol uygulandı (`uwebr-app::GpuContext`): `Rgba8Unorm` + `STORAGE_BINDING | TEXTURE_BINDING` ara texture → `wgpu::util::TextureBlitter` → surface. Surface formatı da non-sRGB seçiliyor.

---

## Gerçekleşen Modül Yapısı

```text
crates/uwebr-render/src/
├── lib.rs              # mod tanımları + re-export'lar
├── renderer.rs         # Sahne birleştirici (GPU state YOK)
├── scene.rs            # RenderScene, RenderNode, RenderStyle, Background
├── scene_builder.rs    # PositionedNode → vello Scene (fill + draw_glyphs)
├── layout.rs           # Element → TaffyTree<NodeContext> → PositionedNode
├── text.rs             # Parley layout + measure (+ fontsuz tahmin)
├── color.rs            # CSS Color → peniko::Color
├── stylebook.rs        # FAZ 4.5: CSS eşleştirme (tag/class/id) + StyleMask
└── paint.rs            # FAZ 8: ResolvedPaint — renk/font kalıtımı
```

Örnekler (tanılama):

```bash
cargo run -p uwebr-render --example glyph_probe    # glyph üretimi + ölçüm
cargo run -p uwebr-render --example layout_probe   # font-size → text box
```

## Draw Call Eşleştirmeleri (gerçekleşen)

| RenderNodeKind | Vello draw call |
|---|---|
| `Rect` | `scene.fill(Fill::NonZero, IDENTITY, brush, None, &Rect::new(...))` |
| `RoundRect { radius }` | `scene.fill(..., &RoundedRect::new(...))` |
| `Text { content, font_size, color, font_family }` | parley `Layout` → `line.items()` → `scene.draw_glyphs(font).font_size(..).brush(color).draw(glyphs)` |
| `Container` | `background` varsa fill; `overflow_hidden` ise `push_clip_layer` / `pop_layer` |
| `Image` | henüz `Rect` olarak çiziliyor (görsel dekodlama yok) |

`opacity < 1` → `push_layer(Fill::NonZero, Compose::SrcOver, opacity, ...)` / `pop_layer`.

## Gradient Dönüşümü (gerçekleşen)

| Background | Vello |
|---|---|
| `Solid(color)` | `peniko::Brush::Solid` |
| `LinearGradient { start, end, stops }` | `Gradient::new_linear(start, end).with_stops(stops)` |
| `RadialGradient { center, radius, stops }` | `Gradient::new_radial(center, radius).with_stops(stops)` |

**Not:** `Background::LinearGradient` / `RadialGradient` varyantları ve `make_brush` desteği var, ancak CSS tarafı henüz gradient üretmiyor — `uwebr-css` `linear-gradient(...)` değerini `Keyword` olarak saklıyor ve `PaintProps` onu yok sayıyor. Yani gradient yolu şu an yalnız elle kurulan `RenderNode`'larla erişilebilir.

## Bilinen Eksikler

- **`RenderNodeKind::Image`** çiziminde gerçek görsel yok; `Rect` olarak düşüyor.
- **`RenderStyle::overflow_hidden`** sahne tarafında kırpıyor (`push_clip_layer`), ancak `pipeline.rs::paint_to_render_style` bunu her zaman `false` yazıyor — CSS `overflow: hidden` taffy `Style.overflow`'a gidiyor ama boyaya taşınmıyor.
- **Gradient** CSS'ten gelmiyor (yukarıya bkz).
- **Text kırpma/eliding** yok; taşan metin kutu dışına çizilir (üst düzeyde `overflow` kırpması bağlanana kadar).
