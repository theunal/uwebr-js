# FAZ 10: CSS Düzeltmeleri

> Durum: 📋 Plan hazır, onay bekliyor
> Oluşturma: 29 Ağustos 2026
> Karmaşıklık: Basit–Orta | Tahmini: ~2 saat

## Genel Bakış

Üç CSS sorunu: `overflow: hidden` bağlanmamış, gradient parsedız, `vw`/`vh` yüzdeye indirgenmiş. Mevcut altyapının çoğu zaten çalışıyor — sorun veriyi doğru yerden doğru yere taşımamak.

---

## ADIM 1: `overflow: hidden` Düzeltmesi (Basit, ~20 dk)

**Sorun:** CSS `overflow: hidden` Taffy'ye gidiyor ama `ResolvedPaint`'e hiç taşınmıyor. `pipeline.rs:209` her zaman `overflow_hidden: false` yazıyor. Scene tarafı (`push_clip_layer`) zaten çalışıyor.

**Mevcut kırık zincir:**

```
CSS parse     → taffy::Style.overflow   ✅ zaten var
StyleMask     → overflow: bool          ✅ zaten var
PositionedNode → overflow_hidden: bool  ❌ eksik
paint_to_render_style()                 ❌ hardcoded false
RenderStyle.overflow_hidden             ✅ zaten var
push_clip_layer                         ✅ zaten var
```

### 1.1 `PositionedNode`'a overflow ekle

**Dosya:** `crates/uwebr-render/src/layout.rs`

```rust
pub struct PositionedNode {
    pub taffy_node: taffy::NodeId,
    pub element: Element,
    pub layout: LayoutInfo,
    pub depth: usize,
    pub paint: ResolvedPaint,
    pub overflow_hidden: bool,  // ← yeni
}
```

### 1.2 `collect_recursive`'de overflow oku

**Dosya:** `crates/uwebr-render/src/layout.rs`

```rust
let taffy_style = self.taffy.style(taffy_node)?;
let overflow_hidden = taffy_style.overflow.x == taffy::style::Overflow::Hidden
    || taffy_style.overflow.y == taffy::style::Overflow::Hidden;
```

### 1.3 `paint_to_render_style`'ı güncelle

**Dosya:** `crates/uwebr-app/src/pipeline.rs`

```rust
fn paint_to_render_style(paint: &ResolvedPaint, overflow_hidden: bool) -> RenderStyle {
    RenderStyle {
        background: paint.background.map(Background::Solid),
        border: /* ... */,
        border_radius: paint.border_radius,
        opacity: paint.opacity,
        overflow_hidden,  // ← hardcoded false yerine parametre
    }
}
```

`positioned_to_render_node` çağrısını güncelle:

```rust
style: paint_to_render_style(&pos.paint, pos.overflow_hidden),
```

### 1.4 Testler

- `layout.rs`: `overflow_hidden` alanının doğru dolduğunu doğrula
- `pipeline.rs`: `overflow:hidden` CSS'i clip layer ürettiğini doğrula

---

## ADIM 2: Gradient Desteği (Orta, ~60 dk)

**Sorun:** `linear-gradient(red, blue)` → `CssValue::Keyword("linear-gradient(red, blue)")` → `extract_paint`'de yok sayılıyor. Render tarafı (`Background::LinearGradient` + `make_brush`) zaten çalışıyor.

**Mevcut kırık zincir:**

```
CSS parser     → CssValue::LinearGradient   ❌ eksik
CSS AST        → gradient varyantı           ❌ eksik
PaintProps     → background enum             ❌ eksik (Color-only)
ResolvedPaint  → background enum             ❌ eksik (Color-only)
pipeline.rs    → Background mapping          ❌ Solid-only
scene.rs       → Background::LinearGradient  ✅ zaten var
scene_builder  → make_brush gradient         ✅ zaten var
```

### 2.1 AST'ye gradient tipleri ekle

**Dosya:** `crates/uwebr-css/src/ast.rs`

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct GradientStop {
    pub color: Color,
    pub position: Option<f32>,  // None → otomatik dağılım
}

#[derive(Debug, Clone, PartialEq)]
pub enum CssValue {
    Length(f32, LengthUnit),
    Color(Color),
    Keyword(String),
    LinearGradient {
        direction: Option<String>,  // "to right", "45deg", None → default
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        stops: Vec<GradientStop>,
    },
    Shorthand(Vec<CssValue>),
    Inherited,
    Auto,
}
```

### 2.2 Parser'da gradient ayrıştırma

**Dosya:** `crates/uwebr-css/src/parser.rs`

`parse_single_value` fonksiyonunda keyword fallback'den önce:

```rust
if raw.starts_with("linear-gradient(") {
    return parse_linear_gradient(raw);
}
if raw.starts_with("radial-gradient(") {
    return parse_radial_gradient(raw);
}
```

Yeni fonksiyonlar:

```rust
fn parse_linear_gradient(raw: &str) -> Result<CssValue> {
    // Parantez içini ayıkla
    // Yön: "to right", "to bottom", "45deg" (None → default: to bottom)
    // Dur'ları ayrıştır: "red 0%", "blue 100%", "red"
    // Her dur'u GradientStop'a çevir
}

fn parse_radial_gradient(raw: &str) -> Result<CssValue> {
    // Benzer şekilde, yön yok
}
```

**Ayrıştırma kuralları:**
- `linear-gradient(to right, red, blue)` → direction: `Some("to right")`, stops: `[{red, None}, {blue, None}]`
- `linear-gradient(45deg, #ff0000, rgb(0,0,255))` → yön + renk parse
- `linear-gradient(red 0%, blue 50%, green 100%)` → pozisyonlu dur'lar
- Geçersiz gradient → `Keyword` olarak düşsün (mevcut davranışı koru)

### 2.3 `PaintProps`'ı genişlet

**Dosya:** `crates/uwebr-css/src/codegen.rs`

```rust
// Yeni enum:
pub enum BackgroundValue {
    Solid(Color),
    LinearGradient {
        direction: Option<String>,
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        stops: Vec<GradientStop>,
    },
}

// PaintProps güncellemesi:
pub background: Option<BackgroundValue>,  // ← Option<Color> yerine
```

`extract_paint` fonksiyonunu güncelle:

```rust
"background" | "background-color" => {
    match &prop.value {
        CssValue::Color(c) => {
            paint.background = Some(BackgroundValue::Solid(c.clone()));
        }
        CssValue::LinearGradient { direction, stops } => {
            paint.background = Some(BackgroundValue::LinearGradient {
                direction: direction.clone(),
                stops: stops.clone(),
            });
        }
        CssValue::RadialGradient { stops } => {
            paint.background = Some(BackgroundValue::RadialGradient {
                stops: stops.clone(),
            });
        }
        _ => {}
    }
}
```

### 2.4 `ResolvedPaint`'i güncelle

**Dosya:** `crates/uwebr-render/src/paint.rs`

```rust
pub background: Option<scene::Background>,  // ← Option<peniko::Color> yerine
```

`apply_css` fonksiyonunu güncelle — gradient durumunu `scene::Background`'a dönüştür:

```rust
fn background_to_scene(bg: &BackgroundValue) -> scene::Background {
    match bg {
        BackgroundValue::Solid(c) => scene::Background::Solid(css_color_to_peniko(c)),
        BackgroundValue::LinearGradient { direction, stops } => {
            let (start, end) = parse_gradient_direction(direction);
            scene::Background::LinearGradient {
                start,
                end,
                stops: stops.iter().map(|s| {
                    (s.position.unwrap_or(0.5), css_color_to_peniko(&s.color))
                }).collect(),
            }
        }
        BackgroundValue::RadialGradient { stops } => {
            scene::Background::RadialGradient {
                center: [0.5, 0.5],
                radius: 0.5,
                stops: stops.iter().map(|s| {
                    (s.position.unwrap_or(0.5), css_color_to_peniko(&s.color))
                }).collect(),
            }
        }
    }
}
```

### 2.5 Pipeline güncellemesi

**Dosya:** `crates/uwebr-app/src/pipeline.rs`

```rust
// Mevcut (satır 198):
background: paint.background.map(Background::Solid),

// Yeni:
background: paint.background.clone(),  // zaten scene::Background tipinde
```

### 2.6 Gradient yön ayrıştırma

**Dosya:** `crates/uwebr-render/src/paint.rs`

```rust
fn parse_gradient_direction(direction: &Option<String>) -> ([f32; 2], [f32; 2]) {
    match direction.as_deref() {
        Some("to right") => ([0.0, 0.0], [1.0, 0.0]),
        Some("to left") => ([1.0, 0.0], [0.0, 0.0]),
        Some("to bottom") => ([0.0, 0.0], [0.0, 1.0]),
        Some("to top") => ([0.0, 1.0], [0.0, 0.0]),
        Some(deg_str) if deg_str.ends_with("deg") => {
            let deg: f32 = deg_str.trim_end_matches("deg").parse().unwrap_or(0.0);
            let rad = deg.to_radians();
            ([0.5 - 0.5 * rad.cos(), 0.5 + 0.5 * rad.sin()],
             [0.5 + 0.5 * rad.cos(), 0.5 - 0.5 * rad.sin()])
        }
        _ => ([0.0, 0.0], [0.0, 1.0]), // default: to bottom
    }
}
```

### 2.7 Testler

- `parser.rs`: `linear-gradient(red, blue)` doğru ayrıştırılıyor mu?
- `parser.rs`: `linear-gradient(to right, red 0%, blue 100%)` yön + pozisyonlu dur
- `parser.rs`: `radial-gradient(red, blue)` ayrıştırma
- `codegen.rs`: `extract_paint` gradient → `BackgroundValue::LinearGradient`
- `paint.rs`: `background_to_scene` doğru `Background::LinearGradient` üretiyor
- `scene_builder.rs`: gradient brush testi (zaten mevcut)

---

## ADIM 3: `vw`/`vh` Düzeltmesi (Orta-Karmaşık, ~40 dk)

**Sorun:** `50vw` → `percent(0.5)` → ebeveyne göre çözülüyor. Doğrusu: viewport'a göre çözülmeli.

**Not:** Kök seviyede zaten doğru çalışır (root %100 viewport'a eşit). İç içe elementlerde yanlış.

### 3.1 Yaklaşım: StyleBook'u viewport boyutlarıyla yeniden çöz

**Dosya:** `crates/uwebr-css/src/codegen.rs`

`to_length_percentage`, `to_length_percentage_auto`, `to_dimension` fonksiyonlarına viewport parametresi ekle:

```rust
fn to_length_percentage(val: &CssValue, vw: f32, vh: f32) -> Option<LengthPercentage> {
    match val {
        CssValue::Length(n, unit) => match unit {
            LengthUnit::Vw => Some(LengthPercentage::length(n / 100.0 * vw)),
            LengthUnit::Vh => Some(LengthPercentage::length(n / 100.0 * vh)),
            LengthUnit::Percent => Some(LengthPercentage::percent(*n / 100.0)),
            _ => Some(LengthPercentage::length(*n)),
        },
        _ => None,
    }
}
```

Aynı değişiklik `to_length_percentage_auto` ve `to_dimension`'a da uygulanır.

### 3.2 `convert_to_style_entries`'e viewport parametresi

**Dosya:** `crates/uwebr-css/src/codegen.rs`

```rust
pub fn convert_to_style_entries(
    rules: &[CssRule],
    viewport_width: f32,
    viewport_height: f32,
) -> Result<Vec<StyleEntry>> {
    // ... mevcut kod, ama to_* fonksiyonlarına viewport geçir
}
```

### 3.3 `StyleBook`'u güncelle

**Dosya:** `crates/uwebr-render/src/stylebook.rs`

```rust
impl StyleBook {
    pub fn parse(css: &str, vw: f32, vh: f32) -> anyhow::Result<Self> {
        let rules = parse_css(css)?;
        Ok(Self {
            rules: convert_to_style_entries(&rules, vw, vh)?,
        })
    }

    pub fn reparse(&mut self, css: &str, vw: f32, vh: f32) -> anyhow::Result<()> {
        let rules = parse_css(css)?;
        self.rules = convert_to_style_entries(&rules, vw, vh)?;
        Ok(())
    }
}
```

### 3.4 Pipeline'da viewport geçir

**Dosya:** `crates/uwebr-app/src/pipeline.rs`

`RenderPipeline`'a `css_string` alanı ekle:

```rust
pub struct RenderPipeline {
    // ... mevcut alanlar
    css_string: Option<String>,  // ← yeni
}
```

`build_render_scene` fonksiyonunda:

```rust
pub fn build_render_scene(&mut self, element: &Element, width: u32, height: u32) {
    if let Some(ref css) = self.css_string {
        let _ = self.stylebook.reparse(css, width as f32, height as f32);
    }
    // ... mevcut kod
}
```

### 3.5 Testler

- `codegen.rs`: `50vw` 800px viewport'ta `length(400)` üretiyor mu?
- `codegen.rs`: `50vh` 600px viewport'ta `length(300)` üretiyor mu?
- `stylebook.rs`: `reparse` yeni boyutlarla çözüyor mu?
- Entegrasyon: kök `100vh` hala doğru mu, iç içe `50vw` artık viewport'a göre mi?

---

## Sıralama

| Sıra | Adım | Karmaşıklık | Tahmini | Dosya Sayısı |
|------|------|-------------|---------|-------------|
| 1 | overflow:hidden | Basit | ~20 dk | 2 |
| 2 | gradient | Orta | ~60 dk | 5 |
| 3 | vw/vh | Orta-Karmaşık | ~40 dk | 4 |

**Toplam tahmini:** ~120 dakika (~2 saat)
**Beklenen test artışı:** +16 test (4+8+4)

---

## Beklenen Sonuç

- `overflow: hidden` CSS'i真正 clip layer üretir
- `background: linear-gradient(red, blue)` ekranda gradient olarak görünür
- `50vw` iç içe elementlerde viewport'a göre çözülür (800px → 400px)
- Eski davranışlar korunur (solid color, root vw/vh)
