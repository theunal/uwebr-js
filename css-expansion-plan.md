# CSS Desteği Genişletme Planı

## Genel Mimarisi

```
CSS String → Parser → AST → apply_property() → Style + Mask + PaintProps + TransformProps
                                                                      ↓
                                              Taffy Layout ← taffy::Style
                                                                      ↓
                                              Vello Scene ← ResolvedPaint + Position + Transform
```

Her özellik üç katman etkiliyor:
1. **codegen.rs** — CSS property → taffy/transform/paint dönüşümü
2. **layout.rs** — Taffy'ye_Style geçirilmesi
3. **scene_builder.rs** — Vello'a çizim

---

## HAFTA 1: Hızlı Kazanımlar (5 gün)

### 1.1 flex-basis (1 gün)

**Dosya:** `crates/uwebr-css/src/codegen.rs`
- `StyleMask.flex_basis` zaten var (satır 18) ama hiç set edilmiyor
- `apply_property()`'ye `"flex-basis"` case'i ekle (satır ~400 civarı)
- `to_dimension()` helper'ı zaten var — aynısını kullan

```rust
"flex-basis" => {
    style.flex_basis = to_dimension(value);
    mask.flex_basis = true;
}
```

**Dosya:** `crates/uwebr-css/src/codegen.rs` — test ekle

**Test:** `flex-basis: 200px` → `style.flex_basis == Dimension::Length(200.0)`

### 1.2 align-content (1 gün)

**Dosya:** `crates/uwebr-css/src/codegen.rs`
- `StyleMask`'e `align_content: bool` ekle
- `apply_property()`'ye `"align-content"` case'i ekle
- `to_align_content()` helper fonksiyonu yaz (justify_content ile aynı değerler + stretch, start/end)

```rust
"align-content" => {
    style.align_content = to_align_content(value);
    mask.align_content = true;
}
```

**Dosya:** `crates/uwebr-render/src/stylebook.rs`
- `merge_style()`'e `align_content` alanını ekle (satır ~610 civarı)

**Test:** `align-content: space-between` → `style.align_content == Some(AlignContent::SpaceBetween)`

### 1.3 z-index (1 gün)

**Dosya:** `crates/uwebr-css/src/codegen.rs`
- `StyleMask`'e `z_index: bool` ekle
- `PaintProps`'e `z_index: Option<i32>` ekle (layout ile ilgili ama paint sırasını etkiliyor)
- `apply_property()`'ye `"z-index"` case'i ekle

```rust
"z-index" => {
    paint.z_index = value.value.parse::<i32>().ok();
    mask.z_index = true;
}
```

**Dosya:** `crates/uwebr-render/src/scene_builder.rs`
- `PositionedNode`'a `z_index: i32` ekle
- `collect_positioned_nodes()`'de z-index'i topla
- `build()`'te node'ları z-index'e göre sırala (düşük → yüksek, yüksek üstte)

**Dosya:** `crates/uwebr-render/src/layout.rs`
- `collect_positioned_nodes()`'de `ResolvedPaint`'ten z_index'i oku

**Test:** `z-index: 10` → node更高de çizilir

### 1.4 Grid Wiring (2 gün)

**Dosya:** `crates/uwebr-css/src/codegen.rs`
- `StyleMask`'e grid alanları ekle:
  ```rust
  pub grid_template_columns: bool,
  pub grid_template_rows: bool,
  pub grid_column: bool,
  pub grid_row: bool,
  pub grid_area: bool,
  ```

- `apply_property()`'ye grid case'leri ekle:

```rust
"grid-template-columns" => {
    // "1fr 2fr 1fr" → vec![TrackSizing::Flexible(1.0), ...]
    style.grid_template_columns = parse_grid_tracks(value);
    mask.grid_template_columns = true;
}
"grid-template-rows" => {
    style.grid_template_rows = parse_grid_tracks(value);
    mask.grid_template_rows = true;
}
"grid-column" => {
    // "1 / 3" → GridPlacement::span(2) veya line-based
    style.grid_column = parse_grid_placement(value);
    mask.grid_column = true;
}
"grid-row" => {
    style.grid_row = parse_grid_placement(value);
    mask.grid_row = true;
}
```

- Helper fonksiyonlar:
  ```rust
  fn parse_grid_tracks(value: &CssValue) -> Option<Vec<TrackSizing>> { ... }
  fn parse_grid_placement(value: &CssValue) -> Option<GridPlacement> { ... }
  ```

**Dosya:** `crates/uwebr-render/src/stylebook.rs`
- `merge_style()`'e grid alanlarını ekle

**Dosya:** `crates/uwebr-render/src/layout.rs`
- `element_to_style()`'de tag varsayılanlarını ekle:
  ```rust
  // div → display: flex (fallback olarak grid değil)
  // section → display: flex
  ```

**Testler:**
```css
.container { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 8px; }
.item { grid-column: 1 / 3; }
```

### 1.5 position: fixed/sticky (1 gün)

**Dosya:** `crates/uwebr-css/src/codegen.rs`
- `to_position()`'a `fixed` ve `sticky` ekle

```rust
fn to_position(value: &CssValue) -> Option<taffy::Position> {
    match value.keyword()? {
        "relative" => Some(taffy::Position::Relative),
        "absolute" => Some(taffy::Position::Absolute),
        "fixed" => Some(taffy::Position::Absolute), // fixed = viewport-relative absolute
        "sticky" => Some(taffy::Position::Relative), // sticky = scroll-aware relative (fallback)
        _ => None,
    }
}
```

**Not:** `fixed` asıl = viewport relative. Buamelde:
1. `position: fixed` → `Position::Absolute`
2. Layout hesaplanırken root'un scroll offset'i eklenir (viewport coords)
3. Scroll sırasında sabit kalması için `collect_positioned_nodes()`'de scroll offset uygulanmaz

**Sticky için:**
1. `position: sticky` → `Position::Relative` (fallback)
2. Gerçek sticky behavior scroll logic gerektirir — ileride eklenebilir

**Test:** `position: fixed; top: 0;` → viewport'un en üstünde sabit kalır

---

## HAFTA 2-3: Orta Zorluk (10 gün)

### 2.1 Transform (5 gün)

**Dosya:** `crates/uwebr-css/src/codegen.rs`
- Yeni struct:
  ```rust
  #[derive(Debug, Default, Clone)]
  pub struct TransformProps {
      pub translate_x: Option<f32>,
      pub translate_y: Option<f32>,
      pub rotate: Option<f32>,        // derece
      pub scale_x: Option<f32>,
      pub scale_y: Option<f32>,
  }
  ```

- `StyleEntry`'ye `transform: TransformProps` ekle
- `extract_transform()` fonksiyonu yaz:
  ```rust
  fn extract_transform(props: &[CssProperty]) -> TransformProps {
      let mut t = TransformProps::default();
      for prop in props {
          match prop.name.as_str() {
              "transform" => { /* parse translate/rotate/scale */ }
              "translate-x" => { t.translate_x = parse_length(&prop.value); }
              "translate-y" => { t.translate_y = parse_length(&prop.value); }
              "rotate" => { t.rotate = parse_angle(&prop.value); }
              "scale" => { /* parse scale(x, y) */ }
              _ => {}
          }
      }
      t
  }
  ```

- `transform` shorthand parsing:
  ```
  transform: translateX(10px) rotate(45deg) scale(1.5)
  → translate_x: Some(10.0), rotate: Some(45.0), scale_x: Some(1.5), scale_y: Some(1.5)
  ```

**Dosya:** `crates/uwebr-render/src/scene.rs`
- `RenderNode`'a `transform: Option<TransformProps>` ekle

**Dosya:** `crates/uwebr-render/src/scene_builder.rs`
- `draw_node()`'de transform uygula:
  ```rust
  if let Some(t) = &node.transform {
      scene.push_transform(
          Affine::translate(t.translate_x.unwrap_or(0.0) as f64, t.translate_y.unwrap_or(0.0) as f64)
          * Affine::rotate(t.rotate.unwrap_or(0.0).to_radians())
          * Affine::scale_non_uniform(t.scale_x.unwrap_or(1.0) as f64, t.scale_y.unwrap_or(1.0) as f64)
      );
  }
  // children çiz
  if let Some(t) = &node.transform {
      scene.pop_transform();
  }
  ```

**Dosya:** `crates/uwebr-render/src/layout.rs`
- Transform layout'u etkilemez (sadece görsel), ama taffy'ye geçirilmez

**Testler:**
```css
.card:hover { transform: translateY(-4px); }
.spinner { rotate: 90deg; }
.icon { transform: scale(1.2); }
.combined { transform: translateX(10px) rotate(45deg); }
```

### 2.2 Opacity Compositing (3 gün)

**Dosya:** `crates/uwebr-render/src/paint.rs`
- `ResolvedPaint`'e `opacity: f32` ekle (zaten var, kontrol et)

**Dosya:** `crates/uwebr-render/src/paint.rs`
- `inherited()` fonksiyonunda opacity çakıştır:
  ```rust
  pub fn inherited(&self, parent: &ResolvedPaint) -> Self {
      Self {
          opacity: self.opacity * parent.opacity, // parent × child
          // ... diğer alanlar
      }
  }
  ```

**Dosya:** `crates/uwebr-render/src/scene_builder.rs`
- Her node için opacity'yi vello group'a uygula:
  ```rust
  if node.opacity < 1.0 {
      scene.push_layer(
          BlendMode::new(Combine::SrcOver, 0, Affine::IDENTITY),
          node.opacity as f64,
          // ...
      );
      // children çiz
      scene.pop_layer();
  }
  ```

**Testler:**
```css
.parent { opacity: 0.5; }
.child { opacity: 0.8; } /* parent-child: 0.5 × 0.8 = 0.4 visible */
.hidden { opacity: 0; }  /* tamamen gizli */
```

---

## HAFTA 4+: İleri Seviye (2-3 hafta)

### 3.1 Transition (1 hafta)

**Dosya:** `crates/uwebr-css/src/codegen.rs`
- Yeni struct:
  ```rust
  #[derive(Debug, Default, Clone)]
  pub struct TransitionProps {
      pub property: String,     // "all", "transform", "opacity"
      pub duration_ms: u32,     // varsayılan: 300
      pub timing: String,       // "ease", "linear", "ease-in-out"
      pub delay_ms: u32,        // varsayılan: 0
  }
  ```

- `StyleEntry`'ye `transitions: Vec<TransitionProps>` ekle
- `extract_transitions()` fonksiyonu:
  ```rust
  fn extract_transitions(props: &[CssProperty]) -> Vec<TransitionProps> {
      props.iter()
          .filter(|p| p.name == "transition")
          .map(|p| parse_transition(&p.value))
          .collect()
  }
  ```

**Dosya:** `crates/uwebr-core/src/anim.rs` (YENİ dosya)
- Animation state machine:
  ```rust
  pub struct Transition {
      pub property: String,
      pub from: f32,
      pub to: f32,
      pub start: Instant,
      pub duration: Duration,
      pub timing: TimingFunction,
  }
  
  pub struct TransitionManager {
      active: Vec<Transition>,
  }
  
  impl TransitionManager {
      pub fn start(&mut self, property: &str, from: f32, to: f32, duration: Duration) { ... }
      pub fn tick(&mut self, now: Instant) -> bool { ... } // true = still animating
      pub fn current_value(&self, property: &str) -> f32 { ... }
  }
  ```

- Timing functions:
  ```rust
  pub enum TimingFunction {
      Linear,
      Ease,
      EaseIn,
      EaseOut,
      EaseInOut,
      CubicBezier(f32, f32, f32, f32),
  }
  
  impl TimingFunction {
      pub fn apply(&self, t: f32) -> f32 {
          match self {
              Self::Linear => t,
              Self::Ease => { /* cubic bezier approx */ }
              // ...
          }
      }
  }
  ```

**Dosya:** `crates/uwebr-render/src/layout.rs`
- Her render döngüsünde transition'ları kontrol et:
  ```rust
  // StyleKit.ownerDocument'ın transition_manager'ını kullan
  if transition_manager.tick(Instant::now()) {
      // animasyon devam ediyor → yeniden çiz
      mark_render_dirty();
  }
  ```

**Dosya:** `crates/uwebr-render/src/scene_builder.rs`
- Transition değerlerini sahneye uygula

**Testler:**
```css
.button { transition: transform 200ms ease; }
.button:hover { transform: translateY(-2px); }
.fade { transition: opacity 300ms ease-in-out; }
```

### 3.2 Animation / @keyframes (1-2 hafta)

**Dosya:** `crates/uwebr-css/src/codegen.rs`
- `@keyframes` parsing ekle:
  ```rust
  pub struct KeyframeRule {
      pub name: String,
      pub keyframes: Vec<Keyframe>,
  }
  
  pub struct Keyframe {
      pub selector: String, // "0%", "100%", "from", "to"
      pub properties: Vec<CssProperty>,
  }
  ```

- `StyleEntry`'ye `animation` alanı ekle:
  ```rust
  pub struct AnimationProps {
      pub name: String,
      pub duration_ms: u32,
      pub timing: String,
      pub iteration_count: Option<u32>, // infinite = None
      pub direction: String,            // "normal", "reverse", "alternate"
      pub fill_mode: String,            // "forwards", "backwards", "both"
  }
  ```

**Dosya:** `crates/uwebr-core/src/anim.rs`
- `AnimationManager` ekle:
  ```rust
  pub struct AnimationManager {
      keyframes: HashMap<String, Vec<Keyframe>>,
      active: Vec<ActiveAnimation>,
  }
  
  pub struct ActiveAnimation {
      pub name: String,
      pub element_id: String,
      pub start: Instant,
      pub duration: Duration,
      pub iteration: u32,
      pub max_iterations: Option<u32>,
  }
  ```

**Dosya:** `crates/uwebr-render/src/scene_builder.rs`
- Her frame'de animation tick → interpolated value → sahneye uygula

**Testler:**
```css
@keyframes spin { from { rotate: 0deg; } to { rotate: 360deg; } }
.spinner { animation: spin 1s linear infinite; }

@keyframes fade-in { from { opacity: 0; } to { opacity: 1; } }
.card { animation: fade-in 300ms ease-out; }
```

### 3.3 Box Shadow (3 gün)

**Dosya:** `crates/uwebr-css/src/codegen.rs`
- `PaintProps`'e `box_shadow: Option<Vec<BoxShadow>>` ekle
- `extract_paint()`'e `box-shadow` parsing ekle

**Dosya:** `crates/uwebr-render/src/scene_builder.rs`
- Vello`draw_shadow()` fonksiyonu:
  ```rust
  fn draw_shadow(scene: &mut Scene, shadow: &BoxShadow, rect: &Rect) {
      let brush = Brush::color(shadow.color);
      let shadow_rect = rect.translate(shadow.offset_x, shadow.offset_y);
      scene.fill(
          Fill::NonZero,
          Affine::IDENTITY,
          &brush,
          None,
          &blur_rect(shadow_rect, shadow.blur_radius),
      );
  }
  ```

**Testler:**
```css
.card { box-shadow: 0 4px 6px rgba(0,0,0,0.1); }
.card:hover { box-shadow: 0 8px 12px rgba(0,0,0,0.15); }
```

### 3.4 Overflow + Scroll (3 gün)

**Dosya:** `crates/uwebr-render/src/scene_builder.rs`
- `overflow: hidden` → clip children
- `overflow: scroll` → scroll container

**Dosya:** `crates/uwebr-core/src/events.rs`
- Scroll event handling
- Wheel event → scroll offset güncelle

**Testler:**
```css
.scroll-container { overflow-y: auto; height: 400px; }
```

### 3.5 Text Align + Typography (2 gün)

**Dosya:** `crates/uwebr-css/src/codegen.rs`
- `PaintProps`'e `text_align`, `line_height`, `letter_spacing` ekle

**Dosya:** `crates/uwebr-render/src/text.rs` veya `scene_builder.rs`
- Parley API ile text alignment uygula

**Testler:**
```css
h1 { text-align: center; }
p { line-height: 1.6; letter-spacing: 0.5px; }
```

---

## Test Stratejisi

Her özellik için:
1. **Unit test** (codegen.rs): CSS string → taffy Style dönüşümü
2. **Integration test** (stylebook.rs): Selector matching + cascade
3. **Visual test** (scene_builder.rs): Vello sahne çıktısı

## Mevcut Testler

```
crates/uwebr-css/src/codegen.rs  → ~30 test
crates/uwebr-render/src/stylebook.rs → ~20 test
crates/uwebr-render/src/layout.rs → ~15 test
crates/uwebr-render/src/scene_builder.rs → ~10 test
```

## Başarı Kriterleri

| Kriter | Hedef |
|--------|-------|
| CSS property desteği | ~30 → ~60+ |
| Flexbox | ~%80 → %95 |
| Grid | %0 → ~%70 |
| Transform | %0 → %90 |
| Opacity compositing | Basit → Full |
| Transition | %0 → ~%80 |
| Toplam test | ~190 → ~250+ |
