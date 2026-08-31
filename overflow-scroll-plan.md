# Overflow + Scroll Detaylı Plan

## Mimari Özet

Mevcut durum:
- `to_overflow()` (codegen.rs:1340) zaten `scroll` → `Overflow::Scroll` dönüştürüyor
- `layout.rs:394` sadece `Hidden/Clip` için `overflow_hidden = true` set ediyor
- `AppEvent::MouseScroll(f32, f32)` var ama event loop'a bağlı değil
- `scene_builder.rs:95` sadece `overflow_hidden` clip uyguluyor
- Pipeline her frame'de sıfırdan build ediyor (`build_render_scene` reset yapıyor)

Hedef:
- `overflow: scroll/auto` olan container'larda çocukları clip + offset ile göster
- Wheel event ile scroll offset güncelle
- Scroll container hit-test ile bulunacak
- Her frame'de scroll offset uygulanarak render

---

## Adım 1: Overflow Modu Ayırımı (codegen.rs + layout.rs)

### codegen.rs — PaintProps
`PaintProps`'e ekle:
```rust
pub overflow_x: Option<String>,  // "visible" | "hidden" | "scroll" | "auto"
pub overflow_y: Option<String>,
```
`extract_paint()`'e ekle:
```rust
"overflow-x" => { paint.overflow_x = value.keyword().map(|s| s.to_string()); mask.overflow_x = true; }
"overflow-y" => { paint.overflow_y = value.keyword().map(|s| s.to_string()); mask.overflow_y = true; }
```
Mevcut `overflow` shorthand'ı ayıklama (zaten var) — tek eksenli `overflow: scroll` → her iki eksene de `scroll` set et.

### paint.rs — ResolvedPaint
```rust
pub overflow_x: Option<String>,
pub overflow_y: Option<String>,
```
`apply_css()`'e ekle, `inherited()`'e ekle.

### scene.rs — RenderStyle
`overflow_hidden: bool` yerine (ya da yanına):
```rust
pub overflow_scroll_x: bool,
pub overflow_scroll_y: bool,
```

### layout.rs — PositionedNode
```rust
pub overflow_scroll_x: bool,
pub overflow_scroll_y: bool,
```
`collect_positioned_nodes()`'de `Overflow::Scroll` kontrolü:
```rust
let overflow_scroll_x = matches!(s.overflow.x, Overflow::Scroll);
let overflow_scroll_y = matches!(s.overflow.y, Overflow::Scroll);
```

---

## Adım 2: Scroll State Depolama (uwebr-app/pipeline.rs)

```rust
use std::collections::HashMap;

/// Bir scroll container'ın kaydırma durumu.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset_x: f32,
    pub offset_y: f32,
}

/// Her pencere için scroll state'ler.
/// Key: node_id (layout sırasında atanan unique id).
pub type ScrollMap = HashMap<usize, ScrollState>;
```

`RenderPipeline`'e ekle:
```rust
scroll_states: ScrollMap,
```

Builder metodu:
```rust
pub fn scroll_offset(&self, node_id: usize) -> ScrollState {
    self.scroll_states.get(&node_id).cloned().unwrap_or_default()
}

pub fn set_scroll_offset(&mut self, node_id: usize, state: ScrollState) {
    self.scroll_states.insert(node_id, state);
}
```

---

## Adım 3: Wheel Event → Scroll State (app.rs)

`window_event()`'e `MouseWheel` handler ekle:
```rust
WindowEvent::MouseWheel { delta, .. } => {
    let (dx, dy) = match delta {
        winit::event::MouseScrollDelta::LineDelta(x, y) => (x * 20.0, y * 20.0),
        winit::event::MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
    };
    state.pipeline.scroll_by(dx, dy);
    state.ctx.window().request_redraw();
}
```

`RenderPipeline`'e `scroll_by` metodu:
```rust
pub fn scroll_by(&mut self, dx: f32, dy: f32) {
    // Basit yaklaşım: scroll _under cursor_ olan ilk scroll container'a uygula
    // Ya da en yakın ancestor'ı bul (hit test ile)
    // İlk versiyon: tüm scroll container'ları kaydır (basit)
    for state in self.scroll_states.values_mut() {
        state.offset_x = (state.offset_x - dx).max(0.0);
        state.offset_y = (state.offset_y - dy).max(0.0);
    }
}
```

---

## Adım 4: Render'da Scroll Offset Uygula (scene_builder.rs)

`draw_node()`'de, scroll container için:
1. Çocukların pozisyonunu scroll offset ile kaydır
2. Clip layer ile sadece container görünür alanında çiz

```rust
// Scroll container clip + offset
if node.style.overflow_scroll_x || node.style.overflow_scroll_y {
    scene.push_clip_layer(
        Fill::NonZero,
        Affine::IDENTITY,
        &Rect::new(x, y, x + w, y + h),
    );

    // Scroll offset uygula
    let sx = if node.style.overflow_scroll_x {
        -scroll_state.offset_x as f64
    } else { 0.0 };
    let sy = if node.style.overflow_scroll_y {
        -scroll_state.offset_y as f64
    } else { 0.0 };

    if sx != 0.0 || sy != 0.0 {
        scene.push_layer(
            Fill::NonZero,
            peniko::Compose::SrcOver,
            1.0,
            Affine::translate((sx, sy)),
            &Rect::new(0.0, 0.0, w + 1000.0, h + 1000.0), // bounds
        );
    }
}

// ... children çiz ...

if node.style.overflow_scroll_x || node.style.overflow_scroll_y {
    if sx != 0.0 || sy != 0.0 {
        scene.pop_layer(); // transform
    }
    scene.pop_layer(); // clip
}
```

**Önemli:** RenderScene yapısı şu an düz liste — scroll offset'i uygulamak için `draw_node`'e scroll state parametresi geçirilmeli. `RenderNode`'e `scroll_state: Option<ScrollState>` eklemek daha temiz olabilir.

---

## Adım 5: Hit-Test ile Scroll Container Bulma

Mevcut `hit_test()` sadece `on:click` action'ları arıyor. Scroll için:

```rust
/// Cursor altındaki scroll container'ı bul (en derin olan).
pub fn scroll_container_at(&self, x: f32, y: f32) -> Option<usize> {
    // element_boxes'ı depth'e göre ters sırada tara
    // İlk scroll_container_x veya scroll_container_y olan node'u döndür
}
```

---

## Adım 6: Scrollbar Çizimi (opsiyonel ama değerli)

Scroll container içeriği taştığında scrollbar çiz:
- Dikey scrollbar: sağ kenar, yükseklik = container_h, thumb = container_h * (container_h / content_h)
- Yatay scrollbar: alt kenar
- Thumb pozisyonu = offset / (content - container) * (track - thumb)

---

## Dosya Değişiklik Özeti

| Dosya | Değişiklik |
|-------|-----------|
| `crates/uwebr-css/src/codegen.rs` | `overflow_x`, `overflow_y` parsing |
| `crates/uwebr-render/src/paint.rs` | `ResolvedPaint` alanları + `apply_css()` |
| `crates/uwebr-render/src/scene.rs` | `RenderStyle` scroll alanları |
| `crates/uwebr-render/src/scene_builder.rs` | Scroll clip + offset uygulama |
| `crates/uwebr-render/src/layout.rs` | `PositionedNode` scroll alanları |
| `crates/uwebr-app/src/pipeline.rs` | `ScrollState`, `ScrollMap`, `scroll_by()` |
| `crates/uwebr-app/src/app.rs` | `MouseWheel` handler |

## Test Stratejisi

1. **codegen**: `overflow-x: scroll` → `overflow_x = Some("scroll")`
2. **layout**: `overflow: scroll` → `overflow_scroll_x = true`, `overflow_scroll_y = true`
3. **pipeline**: `scroll_by(0, 50)` → `scroll_states[node].offset_y == 50.0`
4. **scene_builder**: scroll container clip layer doğru sırada push/pop
5. **integration**: container içinde uzun içerik + wheel event → clip + offset görünür
