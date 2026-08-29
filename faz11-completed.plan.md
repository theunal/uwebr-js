# FAZ 11: Görsel ve Metin İyileştirmeleri

> Durum: 📋 Plan hazır, onay bekliyor
> Oluşturma: 29 Ağustos 2026
> Karmaşıklık: Yüksek | Tahmini: ~3-4 saat

## Genel Bakış

Üç eksik: image desteği (render tarafı stub), text-overflow: ellipsis (hiç yok), {@html expr} (transpile-time çözüm var ama runtime'da boş).

---

## ADIM 1: Image Desteği (~90 dk)

### Mevcut Durum

- `RenderNodeKind::Image { data: Vec<u8>, width: u32, height: u32 }` zaten var (scene.rs:97-101)
- `scene_builder.rs:106-108`: sadece placeholder rect çiziyor:
  ```rust
  RenderNodeKind::Image { .. } => {
      Self::draw_rect(scene, &node.style, x, y, w, h);
  }
  ```
- `image` crate'i Cargo.toml'da yok
- `<img>` tag'i HTML parser'da destekleniyor mu? → kontrol edilecek
- Vello'da `scene.draw_image()` API'si var

### Yapılacaklar

#### 1.1 `image` crate'i ekle

**Dosya:** `crates/uwebr-render/Cargo.toml`

```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
```

#### 1.2 `draw_image` fonksiyonu yaz

**Dosya:** `crates/uwebr-render/src/scene_builder.rs`

`RenderNodeKind::Image` dalını tam implemente et:

```rust
RenderNodeKind::Image { data, width, height } => {
    self.draw_image(scene, data, *width, *height, x, y, w, h);
}
```

Yeni fonksiyon:

```rust
fn draw_image(
    &mut self,
    scene: &mut vello::Scene,
    data: &[u8],
    img_width: u32,
    img_height: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    // 1. image crate ile dekodla
    let img = match image::load_from_memory(data) {
        Ok(img) => img,
        Err(_) => return,  // geçersiz image → sessizce atla
    };

    // 2. Rgba8'e çevir
    let rgba = img.to_rgba8();
    let (iw, ih) = rgba.dimensions();

    // 3. vello Image oluştur
    let vello_img = vello::Image::new(
        rgba.into_raw(),
        vello::ImageFormat::Rgba8,
        iw,
        ih,
    );

    // 4. Vello scene'e çiz (object-fit: contain benzeri)
    scene.draw_image(
        &vello_img,
        Affine::IDENTITY
            * Affine::translate((x, y))
            * Affine::scale_non_uniform(w / iw as f64, h / ih as f64),
    );
}
```

#### 1.3 `RenderNode`'a image helper ekle

**Dosya:** `crates/uwebr-render/src/scene.rs`

```rust
impl RenderNode {
    pub fn image(id: u64, layout: LayoutInfo, data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            id,
            kind: RenderNodeKind::Image { data, width, height },
            layout,
            style: RenderStyle::default(),
        }
    }
}
```

#### 1.4 `<img>` tag desteği (transpile-time)

**Durum:** HTML parser'da `<img>` zaten destekleniyor (self-closing tag). Ama `src` attribute'u Rust expression'ına bağlanmalı.

Transpile-time'da `<img src={logo_bytes} />` → `image(id, layout, logo_bytes, w, h)` dönüşümü gerekiyor.

Buˌu iki şekilde yapabiliriz:
- **Seçenek A (basit):** `<img>` tag'i `NodeType::Element("img")` olarak parse ediliyor, transpiler bunu `RenderNodeKind::Image`'a dönüştürüyor
- **Seçenek B (runtime):** `<img>` tag'i DOM'da bir element olarak kalıyor, layout engine Taffy'de ölçüyor, pipeline image verisini prop'tan alıyor

**Önerilen: Seçenek B** — daha esnek, runtime'da src değişebilir.

**Dosya:** `crates/uwebr-app/src/pipeline.rs`

`NodeType::Element("img")` durumunu handle et:

```rust
NodeType::Element(tag) if tag == "img" => {
    // src prop'undan image verisini al
    let data = element.props.iter()
        .find(|(k, _)| k == "src")
        .and_then(|(_, v)| match v {
            PropValue::String(s) => Some(s.as_bytes().to_vec()),
            _ => None,
        })
        .unwrap_or_default();

    let width = element.props.iter()
        .find(|(k, _)| k == "width")
        .and_then(|(_, v)| match v {
            PropValue::Number(n) => Some(*n as u32),
            _ => None,
        })
        .unwrap_or(0);

    let height = element.props.iter()
        .find(|(k, _)| k == "height")
        .and_then(|(_, v)| match v {
            PropValue::Number(n) => Some(*n as u32),
            _ => None,
        })
        .unwrap_or(0);

    Some(RenderNode {
        id,
        kind: RenderNodeKind::Image { data, width, height },
        layout,
        style: /* resolved paint */,
    })
}
```

**Not:** Bu yaklaşım `src`'yi byte array olarak bekliyor. Gerçek uygulamada dosya okuma veya embedded bytes gerekecek. FAZ 11'de sadece byte array desteğini ekliyoruz, dosya okuma sonraki fazlara.

#### 1.5 Testler

- `scene_builder.rs`: `draw_image` geçerli PNG ile doğru image çiziyor (snapshit veya n_clips kontrolü)
- `scene_builder.rs`: geçersiz byte array → sessizce atlıyor (panic yok)
- `pipeline.rs`: `<img>` tag'i `RenderNodeKind::Image` üretiyor
- `pipeline.rs`: `width`/`height` prop'ları doğru okunuyor

---

## ADIM 2: Text Overflow: Ellipsis (~60 dk)

### Mevcut Durum

- `RenderStyle`'de `text_overflow` alanı yok
- `scene_builder.rs`'de draw_text fonksiyonu metni olduğu gibi çiziyor, kırpma yok
- Vello'da `push_clip_layer` zaten `overflow_hidden` için kullanılıyor
- Parley metni box'a sığdırmak için word-break yapıyor ama ellipsis eklemiyor

### Yaklaşım

Metin clip layer ile kırpılacak, sondaki visible kısma "..." eklenecek.

### Yapılacaklar

#### 2.1 `TextOverflow` enumu ekle

**Dosya:** `crates/uwebr-render/src/scene.rs`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
    Visible,
}

impl Default for TextOverflow {
    fn default() -> Self {
        Self::Clip
    }
}
```

`RenderStyle`'e ekle:

```rust
pub struct RenderStyle {
    pub background: Option<Background>,
    pub border: Option<BorderStyle>,
    pub border_radius: f32,
    pub opacity: f32,
    pub overflow_hidden: bool,
    pub text_overflow: TextOverflow,  // ← yeni
}
```

#### 2.2 CSS'den text-overflow oku

**Dosya:** `crates/uwebr-css/src/codegen.rs`

`PaintProps`'e ekle:

```rust
pub text_overflow: Option<String>,  // "clip", "ellipsis"
```

`extract_paint`'de:

```rust
"text-overflow" => {
    if let CssValue::Keyword(k) = &prop.value {
        paint.text_overflow = Some(k.clone());
    }
}
```

**Dosya:** `crates/uwebr-render/src/paint.rs`

`apply_css`'de:

```rust
if let Some(ref to) = props.text_overflow {
    self.text_overflow = match to.as_str() {
        "ellipsis" => TextOverflow::Ellipsis,
        "clip" => TextOverflow::Clip,
        _ => TextOverflow::Clip,
    };
}
```

#### 2.3 Pipeline'da aktar

**Dosya:** `crates/uwebr-app/src/pipeline.rs`

```rust
fn paint_to_render_style(paint: &ResolvedPaint, overflow_hidden: bool) -> RenderStyle {
    RenderStyle {
        // ... mevcut alanlar
        text_overflow: paint.text_overflow.clone(),  // ← yeni
    }
}
```

#### 2.4 `draw_text`'i güncelle

**Dosya:** `crates/uwebr-render/src/scene_builder.rs`

`draw_text` fonksiyonuna `text_overflow` parametresi ekle:

```rust
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
    text_overflow: &TextOverflow,  // ← yeni
) {
    if content.trim().is_empty() {
        return;
    }

    let max_advance = if width > 0.0 {
        Some(width as f32)
    } else {
        None
    };

    // Ellipsis için: metni kısalt
    let display_content = if *text_overflow == TextOverflow::Ellipsis && width > 0.0 {
        self.truncate_with_ellipsis(content, font_size, font_family, width)
    } else {
        content.to_string()
    };

    let layout = self.text.layout_text(
        &display_content, font_size, font_family, max_advance
    );

    // ... mevcut draw kodu
}
```

Yeni fonksiyon:

```rust
fn truncate_with_ellipsis(
    &mut self,
    content: &str,
    font_size: f32,
    font_family: Option<&str>,
    max_width: f64,
) -> String {
    // Parley ile metni ölç
    let full_layout = self.text.layout_text(
        content, font_size, font_family, Some(max_width as f32)
    );

    // Eğer tek satıra sığıyorsa kırpma yok
    let mut total_width = 0.0f32;
    let mut truncated = String::new();
    let ellipsis = "...";

    for line in full_layout.lines() {
        for item in line.items() {
            if let parley::PositionedLayoutItem::GlyphRun(run) = item {
                for glyph in run.glyphs() {
                    total_width += glyph.advance;
                }
            }
        }
    }

    if total_width <= max_width as f32 {
        return content.to_string();
    }

    // Karakter karakter ekle, "..." için yer bırak
    let ellipsis_layout = self.text.layout_text(
        ellipsis, font_size, font_family, None
    );
    let ellipsis_width: f32 = ellipsis_layout.lines()
        .flat_map(|l| l.items())
        .filter_map(|item| {
            if let parley::PositionedLayoutItem::GlyphRun(run) = item {
                Some(run.glyphs().map(|g| g.advance).sum::<f32>())
            } else { None }
        })
        .sum();

    let available_width = max_width as f32 - ellipsis_width;
    let mut width_so_far = 0.0f32;

    for ch in content.chars() {
        let char_layout = self.text.layout_text(
            &ch.to_string(), font_size, font_family, None
        );
        let char_width: f32 = char_layout.lines()
            .flat_map(|l| l.items())
            .filter_map(|item| {
                if let parley::PositionedLayoutItem::GlyphRun(run) = item {
                    Some(run.glyphs().map(|g| g.advance).sum::<f32>())
                } else { None }
            })
            .sum();

        if width_so_far + char_width > available_width {
            break;
        }
        truncated.push(ch);
        width_so_far += char_width;
    }

    format!("{}{}", truncated, ellipsis)
}
```

**Not:** Bu yaklaşım karakter ölçümü kullanıyor. Daha verimli bir yol: parley'in linebreaking'ini kullanarak hangi karakterlerin sığdığını tespit etmek. Ama FAZ 11 için bu yeterli.

#### 2.5 `draw_node` çağrısını güncelle

**Dosya:** `crates/uwebr-render/src/scene_builder.rs`

```rust
RenderNodeKind::Text { content, font_size, color, font_family } => {
    self.draw_text(scene, content, *font_size, *color, font_family.as_deref(), x, y, w, &node.style.text_overflow);
}
```

#### 2.6 Testler

- `scene.rs`: `TextOverflow` default'u `Clip`
- `scene_builder.rs`: `text_overflow: Ellipsis` olan text node'u "..." ile bitiyor
- `scene_builder.rs`: kısa metin ellipsis'e ihtiyaç duymuyor
- `codegen.rs`: `text-overflow: ellipsis` → `PaintProps.text_overflow`

---

## ADIM 3: {@html expr} Runtime Desteği (~60 dk)

### Mevcut Durum

- `HtmlNode::RawHtml(expr)` HTML AST'de var (html/ast.rs:9)
- Codegen: `rsx!(Raw(expr))` üretiyor (html/codegen.rs:18-19)
- `NodeType::Raw(String)` core'da var (component.rs:21)
- `pipeline.rs:204`: `NodeType::Raw(_) => None` — hiçbir şey yapmıyor!

### Sorun

`{@html userHtml}` transpile edildiğinde `rsx!(Raw(user_html_string))` oluşuyor. Ama pipeline bu node'u yok sayıyor. Runtime'da HTML string'i parse edilmeli ve element tree'ye dönüştürülmeli.

### Yaklaşım

İki seviye:
1. **Compile-time:** `{@html expr}` → `Raw(expr)` transpile ediliyor ✅ zaten var
2. **Runtime:** `NodeType::Raw(html_string)` → mini HTML parser ile parse et → element tree'ye çevir

### Yapılacaklar

#### 3.1 Runtime mini HTML parser

**Dosya:** `crates/uwebr-render/src/html_parse.rs` (yeni dosya)

Küçük, bağımsız bir HTML string parser. Sadece temel destek:
- `<div>`, `<span>`, `<p>` gibi tag'ler
- Attribute'lar: `class="foo"`, `id="bar"`
- Metin content
- Self-closing: `<br>`, `<img>`, `<input>`
- Nested elements

```rust
use uwebr_core::component::{Element, NodeType, PropValue};

/// Parse a runtime HTML string into an Element tree.
///
/// This is a minimal parser for {@html expr} support. It handles:
/// - Opening/closing tags with attributes
/// - Self-closing tags
/// - Text content
/// - Basic attribute parsing (string literals only)
pub fn parse_runtime_html(html: &str) -> Option<Element> {
    let parser = RuntimeHtmlParser::new(html);
    parser.parse_element().ok()
}

struct RuntimeHtmlParser {
    input: Vec<char>,
    pos: usize,
}

impl RuntimeHtmlParser {
    fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    fn parse_element(&mut self) -> Option<Element> {
        // '<' bekle
        self.skip_whitespace();
        if self.peek() != Some('<') { return None; }
        self.advance(); // '<'

        // Tag adını oku
        let tag = self.read_tag_name()?;

        // Attribute'ları oku
        let attrs = self.parse_attributes();

        // Self-closing mi?
        self.skip_whitespace();
        if self.peek() == Some('/') {
            self.advance(); // '/'
            self.expect('>');
            return Some(Element {
                node_type: NodeType::Element(tag),
                props: attrs,
                children: vec![],
            });
        }

        self.expect('>');

        // Children'ları parse et
        let mut children = Vec::new();
        loop {
            self.skip_whitespace();
            if self.starts_with("</") {
                self.advance(); // '<'
                self.advance(); // '/'
                self.read_tag_name(); // closing tag adını oku (doğrulama yapılabilir)
                self.expect('>');
                break;
            }
            if self.peek().is_none() {
                break;
            }

            if self.peek() == Some('<') {
                // Child element
                if let Some(child) = self.parse_element() {
                    children.push(child);
                }
            } else {
                // Metin content
                let text = self.read_text();
                if !text.trim().is_empty() {
                    children.push(Element {
                        node_type: NodeType::Text(text.trim().to_string()),
                        props: vec![],
                        children: vec![],
                    });
                }
            }
        }

        Some(Element {
            node_type: NodeType::Element(tag),
            props: attrs,
            children,
        })
    }

    fn parse_attributes(&mut self) -> Vec<(String, PropValue)> {
        let mut attrs = Vec::new();
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('>') | Some('/') | None => break,
                _ => {}
            }

            let name = self.read_attr_name();
            if name.is_empty() { break; }

            self.skip_whitespace();
            if self.peek() == Some('=') {
                self.advance();
                let value = self.read_attr_value();
                attrs.push((name, PropValue::String(value)));
            } else {
                attrs.push((name, PropValue::Bool(true)));
            }
        }
        attrs
    }

    // ... helper fonksiyonlar: peek, advance, skip_whitespace,
    //     read_tag_name, read_attr_name, read_attr_value, read_text,
    //     starts_with, expect
}
```

**Önemli:** Bu parser basit tutulmalı. Gerçek HTML parsing için `html5ever` gibi crate'ler var ama FAZ 11'de sadece {@html} için minimalist bir parser yeterli. Karmaşık HTML'ler için sonraki fazlarda `html5ever` entegrasyonu yapılabilir.

#### 3.2 Pipeline'da Raw node'u handle et

**Dosya:** `crates/uwebr-app/src/pipeline.rs`

```rust
NodeType::Raw(html) => {
    // Runtime HTML string'ini parse et
    if let Some(parsed_element) = uwebr_render::html_parse::parse_runtime_html(html) {
        // Parse edilen element'i recursively render et
        return self.element_to_render_node(&parsed_element, id, depth);
    }
    None
}
```

**Alternatif (daha basit):** Eğer parse edilemezse raw metin olarak göster:

```rust
NodeType::Raw(html) => {
    if let Some(el) = parse_runtime_html(html) {
        return self.element_to_render_node(&el, id, depth);
    }
    // Fallback: raw metin olarak göster
    Some(RenderNode {
        id,
        kind: RenderNodeKind::Text {
            content: html.clone(),
            font_size: 16.0,
            color: peniko::color::palette::css::WHITE,
            font_family: None,
        },
        layout,
        style: Default::default(),
    })
}
```

#### 3.3 `lib.rs`'e modülü ekle

**Dosya:** `crates/uwebr-render/src/lib.rs`

```rust
pub mod html_parse;
```

#### 3.4 Testler

- `html_parse.rs`: basit `<div>Hello</div>` parse
- `html_parse.rs`: attribute'lu `<span class="x">Text</span>` parse
- `html_parse.rs`: self-closing `<br>` parse
- `html_parse.rs`: nested `<div><span>Inner</span></div>` parse
- `html_parse.rs`: geçersiz HTML → `None`
- `pipeline.rs`: `NodeType::Raw("<div>Hi</div>")` → render node üretiyor
- `pipeline.rs`: `NodeType::Raw("<invalid")` → fallback veya None

---

## Sıralama ve Bağımlılıklar

```
ADIM 1 (image) — bağımsız, Cargo.toml değişikliği gerektiriyor
ADIM 2 (ellipsis) — bağımsız
ADIM 3 ({@html}) — bağımsız, yeni dosya gerektiriyor
```

| Sıra | Adım | Karmaşıklık | Tahmini | Yeni Dosya |
|------|------|-------------|---------|------------|
| 1 | image desteği | Orta | ~90 dk | 0 (değişiklik) |
| 2 | text-overflow: ellipsis | Orta | ~60 dk | 0 (değişiklik) |
| 3 | {@html expr} runtime | Yüksek | ~60 dk | 1 (html_parse.rs) |

**Toplam tahmini:** ~210 dakika (~3.5 saat)
**Beklenen test artışı:** +16 test (4+6+6)

---

## Riskler

1. **Image crate boyutu:** `image` crate'i nispeten ağır. Sadece PNG/JPEG feature'ları açık tutarak minimize ediyoruz.
2. **Ellipsis ölçümü:** Karakter karakter ölçümü yavaş olabilir. Sonraki optimizasyon: parley linebreaking ile batch ölçüm.
3. **Runtime HTML parser:** Minimal parser tüm HTML edge case'lerini handle etmez. {@html} ile güvenli olmayan HTML geldiğinde fallback gerekir.
4. **Güvenlik:** {@html} ile kullanıcıdan gelen HTML XSS açığı yaratabilir. FAZ 11'de sanitization yok, sadece parse.

---

## Beklenen Sonuç

- `<img src={bytes} width={100} height={100}` → ekranda image görünür
- `text-overflow: ellipsis` → taşan metin "..." ile kırpılır
- `{@html "<b>Bold</b>"}` → runtime'da parse edilip bold metin render edilir
- Eski davranışlar korunur (geçersiz image → placeholder, ellipsis yoksa clip, geçersiz html → fallback)
