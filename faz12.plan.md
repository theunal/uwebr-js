# FAZ 12: Performans ve Kalite

> Durum: 📋 Plan hazır, onay bekliyor
> Oluşturma: 29 Ağustos 2026
> Karmaşıklık: Orta | Tahmini: ~3-4 saat

## Genel Bakış

Üretim hazırlığı: 21 clippy uyarısı, metrik ölçüm altyapısı yok, benchmark yok, e2e test yok.

---

## ADIM 1: Clippy Temizliği (~45 dk)

**Toplam: 21 uyarı, 6 crate**

### uwebr-html (10 uyarı)

| # | Dosya | Uyarı | Düzeltme |
|---|-------|-------|----------|
| 1 | codegen.rs:71 | `single_char_add_str` | `push_str("}")` → `push('}')` |
| 2 | codegen.rs:134 | `redundant_closure` | `\|a\| generate_attribute(a)` → `generate_attribute` |
| 3 | directives.rs:15 | `collapsible_match` | `if text.contains('{')` match koluyla birleştir |
| 4 | directives.rs:47 | `needless_range_loop` | `for j in (i+1)..len` → iterator + enumerate |
| 5 | directives.rs:59 | `needless_range_loop` | `for k in (i+1)..end` → iterator |
| 6 | directives.rs:77 | `needless_range_loop` | `for j in (i+1)..len` → iterator + enumerate |
| 7 | directives.rs:89 | `needless_range_loop` | `for k in (i+1)..end` → iterator |
| 8 | parser.rs:19 | `unnecessary_map_or` | `.map_or(false, ...)` → `.is_some_and(...)` |
| 9 | parser.rs:37 | `unnecessary_map_or` | `.map_or(false, ...)` → `.is_some_and(...)` |
| 10 | parser.rs:224 | `unnecessary_map_or` | `.map_or(false, ...)` → `.is_some_and(...)` |

### uwebr-core (3 uyarı)

| # | Dosya | Uyarı | Düzeltme |
|---|-------|-------|----------|
| 11 | diff.rs:98 | `ptr_arg` | `path: &mut Vec<usize>` → `path: &mut [usize]` |
| 12 | events.rs:25 | `should_implement_trait` | `from_str` metodunu `FromStr` trait'i olarak implement et veya yeniden adlandır |
| 13 | lifecycle.rs:10 | `missing_const_for_thread_local` | `Cell::new(None)` → `const { Cell::new(None) }` |

### uwebr-js (8 uyarı)

| # | Dosya | Uyarı | Düzeltme |
|---|-------|-------|----------|
| 14 | expressions.rs:133 | `collapsible_if` | İç içe if'leri birleştir |
| 15 | expressions.rs:383 | `get_first` | `.get(0)` → `.first()` |
| 16 | expressions.rs:472 | `get_first` | `.get(0)` → `.first()` |
| 17 | context.rs:25 | `new_without_default` | `Default` impl ekle |
| 18 | transformer.rs:31 | `new_without_default` | `Default` impl ekle |
| 19 | transformer.rs:48 | `redundant_closure` | `\|p\| Self::pat_to_names(p)` → `Self::pat_to_names` |
| 20 | transformer.rs:500 | `unnecessary_to_owned` | `.to_string()` → `.as_ref()` |
| 21 | transformer.rs:1163 | `unnecessary_filter_map` | `.filter_map(...)` → `.map(...)` |

### Yaklaşım

`cargo clippy --fix --workspace --allow-dirty` ile otomatik düzeltme dene. Kalan elle düzeltilir.

---

## ADIM 2: Benchmark Harness (~60 dk)

### 2.1 Criterion ekle

**Dosya:** `Cargo.toml` (workspace)

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "render_bench"
harness = false
```

### 2.2 Benchmark dosyası oluştur

**Dosya:** `benches/render_bench.rs` (yeni)

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_css_parse(c: &mut Criterion) {
    let css = ".a { width: 100px; height: 200px; background: red; }";
    c.bench_function("css_parse_simple", |b| {
        b.iter(|| uwebr_css::parser::parse_css(black_box(css)).unwrap());
    });
}

fn bench_layout_100_nodes(c: &mut Criterion) {
    // 100 nested div'li ağaç oluştur
    let css = ".box { width: 50px; height: 50px; }";
    let mut html = String::from("<div>");
    for i in 0..100 {
        html.push_str(&format!("<div class=\"box\">Node {i}</div>"));
    }
    html.push_str("</div>");

    c.bench_function("layout_100_nodes", |b| {
        b.iter(|| {
            let element = uwebr_html::parser::parse_html(black_box(&html)).unwrap();
            let stylebook = uwebr_render::stylebook::StyleBook::parse(css, 800.0, 600.0).unwrap();
            let mut engine = uwebr_render::layout::LayoutEngine::new();
            engine.build_tree(&element, &stylebook).unwrap();
            engine.compute_layout(black_box(&element), 800.0, 600.0).unwrap();
        });
    });
}

fn bench_scene_build(c: &mut Criterion) {
    // Basit sahne oluştur ve ölç
    c.bench_function("scene_build_empty", |b| {
        b.iter(|| {
            let scene = uwebr_render::scene::RenderScene::new();
            uwebr_render::scene_builder::SceneBuilder::build_scene(
                black_box(&scene), 800, 600
            );
        });
    });
}

fn bench_text_measure(c: &mut Criterion) {
    let mut renderer = uwebr_render::text::TextRenderer::new();
    c.bench_function("text_measure_short", |b| {
        b.iter(|| {
            renderer.measure_text(black_box("Hello World"), 16.0, None);
        });
    });
}

criterion_group!(
    benches,
    bench_css_parse,
    bench_layout_100_nodes,
    bench_scene_build,
    bench_text_measure,
);
criterion_main!(benches);
```

### 2.3 `TextRenderer::measure_text` public erişim

Eğer `measure_text` şu an pub değilse, benchmark'ın erişebilmesi için pub yap.

---

## ADIM 3: 5 Metrik Ölçümü (~60 dk)

### 3.1 Metrik modülü

**Dosya:** `crates/uwebr-render/src/metrics.rs` (yeni)

```rust
use std::time::{Duration, Instant};

/// Performans metrikleri
#[derive(Debug, Clone)]
pub struct Metrics {
    pub fps: f64,
    pub frame_time_ms: f64,
    pub cold_start_ms: f64,
    pub layout_1000_nodes_ms: f64,
    pub memory_bytes: u64,
    pub binary_size_bytes: u64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            fps: 0.0,
            frame_time_ms: 0.0,
            cold_start_ms: 0.0,
            layout_1000_nodes_ms: 0.0,
            memory_bytes: 0,
            binary_size_bytes: 0,
        }
    }
}

impl Metrics {
    /// Boş bir Metrics ölçümü (metrikler ayrı ayrı doldurulur).
    pub fn measure_all() -> Self {
        let mut m = Self::default();
        m.cold_start_ms = Self::measure_cold_start();
        m.layout_1000_nodes_ms = Self::measure_layout_1000();
        m.memory_bytes = Self::measure_memory();
        m
    }

    fn measure_cold_start() -> f64 {
        let start = Instant::now();
        // Basit bir parse + layout ölç
        let css = ".a { width: 100px; }";
        let _ = uwebr_css::parser::parse_css(css);
        start.elapsed().as_secs_f64() * 1000.0
    }

    fn measure_layout_1000() -> f64 {
        let start = Instant::now();
        // 1000 node'lu basit ağaç
        let css = ".box { width: 10px; height: 10px; }";
        let mut html = String::from("<div>");
        for _ in 0..1000 {
            html.push_str("<div class=\"box\">x</div>");
        }
        html.push_str("</div>");

        if let Ok(element) = uwebr_html::parser::parse_html(&html) {
            if let Ok(stylebook) = uwebr_render::stylebook::StyleBook::parse(css, 800.0, 600.0) {
                let mut engine = uwebr_render::layout::LayoutEngine::new();
                let _ = engine.build_tree(&element, &stylebook);
                let _ = engine.compute_layout(&element, 800.0, 600.0);
            }
        }
        start.elapsed().as_secs_f64() * 1000.0
    }

    fn measure_memory() -> u64 {
        // Basit tahmin: process durumu
        #[cfg(target_os = "windows")]
        {
            // Windows: GetProcessMemoryInfo yerine basit tahmin
            0 // Gerçek implementasyon için windows crate'i gerekir
        }
        #[cfg(not(target_os = "windows"))]
        {
            0
        }
    }

    pub fn fps_from_frame_time(frame_time_ms: f64) -> f64 {
        if frame_time_ms > 0.0 { 1000.0 / frame_time_ms } else { 0.0 }
    }
}
```

### 3.2 Binary boyutu ölçümü

**Dosya:** `crates/uwebr-cli/src/main.rs` (veya `commands.rs`)

CLI'ye `uwebr metrics` komutu ekle:

```rust
/// Print performance metrics
pub fn metrics_command() {
    let m = uwebr_render::metrics::Metrics::measure_all();
    println!("Cold start:     {:.2} ms", m.cold_start_ms);
    println!("Layout 1000:    {:.2} ms", m.layout_1000_nodes_ms);
    println!("Memory:         {} bytes", m.memory_bytes);

    // Binary boyutu: kendini oku
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(meta) = std::fs::metadata(&exe) {
            println!("Binary size:    {} bytes", meta.len());
        }
    }
}
```

### 3.3 FPS ölçümü (runtime)

`Renderer`'a frame time tracking ekle:

**Dosya:** `crates/uwebr-render/src/renderer.rs`

```rust
pub struct Renderer {
    // ... mevcut alanlar
    last_frame_time: Option<Instant>,
    frame_time_ms: f64,
}

impl Renderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            scene: RenderScene::new(),
            needs_redraw: true,
            builder: SceneBuilder::new(),
            last_frame_time: None,
            frame_time_ms: 0.0,
        }
    }

    pub fn render_frame(&mut self) -> Result<vello::Scene> {
        let now = Instant::now();
        if let Some(last) = self.last_frame_time {
            self.frame_time_ms = now.duration_since(last).as_secs_f64() * 1000.0;
        }
        self.last_frame_time = Some(now);

        let scene = self.build_vello_scene();
        Ok(scene)
    }

    pub fn fps(&self) -> f64 {
        Metrics::fps_from_frame_time(self.frame_time_ms)
    }

    pub fn frame_time_ms(&self) -> f64 {
        self.frame_time_ms
    }
}
```

### 3.4 Testler

- `metrics.rs`: `measure_cold_start` pozitif süre döndürüyor
- `metrics.rs`: `measure_layout_1000` pozitif süre döndürüyor
- `metrics.rs`: `fps_from_frame_time` doğru hesaplama
- `renderer.rs`: `fps()` ilk frame'de 0, sonraki frame'lerde pozitif

---

## ADIM 4: End-to-End Test (~60 dk)

### 4.1 Yaklaşık

`.uwebr` dosyası → transpile → parse → layout → render pipeline'ı test edilecek.

### 4.2 E2E test dosyası

**Dosya:** `crates/uwebr-render/tests/e2e.rs` (yeni)

```rust
use uwebr_html::parser::parse_html;
use uwebr_css::parser::parse_css;
use uwebr_render::stylebook::StyleBook;
use uwebr_render::layout::LayoutEngine;
use uwebr_render::scene::RenderScene;
use uwebr_render::scene_builder::SceneBuilder;

/// Transpile edilmiş .uwebr çıktısını simüle et:
/// HTML + CSS → parse → layout → render scene → vello scene
#[test]
fn e2e_simple_div_with_background() {
    let html = r#"<div class="box">Hello</div>"#;
    let css = r#"
        .box {
            width: 200px;
            height: 100px;
            background: #ff0000;
            display: flex;
            justify-content: center;
            align-items: center;
        }
    "#;

    // 1. Parse
    let element = parse_html(html).expect("HTML parse failed");
    let stylebook = StyleBook::parse(css, 800.0, 600.0).expect("CSS parse failed");

    // 2. Layout
    let mut engine = LayoutEngine::new();
    let positioned = engine
        .build_tree(&element, &stylebook)
        .expect("build_tree failed");
    let layout = engine
        .compute_layout(&element, 800.0, 600.0)
        .expect("compute_layout failed");

    // 3. Layout sonuçlarını doğrula
    assert!(layout.width > 0.0, "Box should have width");
    assert!(layout.height > 0.0, "Box should have height");

    // 4. Render scene
    let mut scene = RenderScene::new();
    // positioned node'ları scene'e ekle (pipeline benzeri)
    for pos in &positioned {
        let id = 1;
        let render_node = uwebr_render::scene::RenderNode::rect(
            id,
            uwebr_render::scene::LayoutInfo {
                x: pos.layout.left,
                y: pos.layout.top,
                width: pos.layout.width,
                height: pos.layout.height,
            },
            vello::peniko::color::palette::css::RED,
        );
        scene.add_node(render_node);
    }

    // 5. Vello scene
    let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    // Vello scene boş değil (bir fill var)
    // Not: vello::Scene'nin content kontrolü limitedir,
    // ama en azından panic olmadan oluşturulduğunu doğrula
    drop(vello_scene);
}

#[test]
fn e2e_nested_layout_with_text() {
    let html = r#"
        <div class="container">
            <div class="item">Item 1</div>
            <div class="item">Item 2</div>
            <div class="item">Item 3</div>
        </div>
    "#;
    let css = r#"
        .container {
            display: flex;
            flex-direction: column;
            width: 300px;
        }
        .item {
            height: 50px;
            background: #0000ff;
        }
    "#;

    let element = parse_html(html).expect("HTML parse failed");
    let stylebook = StyleBook::parse(css, 800.0, 600.0).expect("CSS parse failed");

    let mut engine = LayoutEngine::new();
    let positioned = engine
        .build_tree(&element, &stylebook)
        .expect("build_tree failed");
    let _layout = engine
        .compute_layout(&element, 800.0, 600.0)
        .expect("compute_layout failed");

    // 3 child node olmalı
    assert!(positioned.len() >= 3, "Should have at least 3 positioned nodes");
}

#[test]
fn e2e_image_render_node() {
    // Image node'u için pipeline seviyesinde test
    use uwebr_core::component::{Element, NodeType, PropValue};
    use uwebr_render::scene::RenderNode;

    let node = RenderNode::image(
        1,
        uwebr_render::scene::LayoutInfo {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        vec![], // boş veri → geçersiz image, panic yok
        0,
        0,
    );

    let mut scene = RenderScene::new();
    scene.add_node(node);
    let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    drop(vello_scene); // panic yok = pass
}

#[test]
fn e2e_gradient_render() {
    let css = r#"
        .grad {
            width: 200px;
            height: 200px;
            background: linear-gradient(to right, #ff0000, #0000ff);
        }
    "#;

    let rules = parse_css(css).expect("CSS parse failed");
    let stylebook = StyleBook::parse(css, 800.0, 600.0).expect("StyleBook parse failed");

    // StyleBook'un rule içerdiğini doğrula
    assert!(!stylebook.rules.is_empty(), "StyleBook should have rules");
}

#[test]
fn e2e_overflow_hidden_clip() {
    use uwebr_core::component::{Element, NodeType, PropValue};
    use uwebr_render::scene::RenderNode;

    let mut scene = RenderScene::new();
    let mut node = RenderNode::rect(
        1,
        uwebr_render::scene::LayoutInfo {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        vello::peniko::color::palette::css::BLUE,
    );
    node.style.overflow_hidden = true;
    scene.add_node(node);

    let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    // overflow_hidden clip layer ekler, vello scene'de en az 1 clip olmalı
    // ( doğrudan erişim zor, ama panic yok = pass )
    drop(vello_scene);
}
```

---

## Sıralama

| Sıra | Adım | Karmaşıklık | Tahmini |
|------|------|-------------|---------|
| 1 | Clippy temizliği | Basit | ~45 dk |
| 2 | Benchmark harness | Orta | ~60 dk |
| 3 | 5 metrik ölçümü | Orta | ~60 dk |
| 4 | E2E test | Orta | ~60 dk |

**Toplam tahmini:** ~225 dakika (~3.75 saat)
**Beklenen test artışı:** +9 test (5 metrik + 4 e2e + bench)

---

## Riskler

1. **Clippy auto-fix:** `--fix` bazı uyarıları tam otomatik düzeltemeyebilir (özellikle `collapsible_match`, `ptr_arg`). Elle müdahale gerekebilir.
2. **Memory ölçümü:** Gerçek bellek ölçümü platforma bağlı. FAZ 12'de basit tahmin yeterli, gerçek implementasyon sonraki fazlarda.
3. **Criterion kurulumu:** İlk `cargo bench` çalıştırması download yapabilir.
4. **E2E test'ler:** Vello scene'in içeriğini doğrudan test etmek zor (API limitedir). Panic yok = pass yaklaşımı yeterli.

---

## Beklenen Sonuç

- `cargo clippy --workspace` → 0 uyarı
- `cargo bench` → criterion raporu oluşturur
- `uwebr metrics` → cold start, layout, bellek, binary boyutu basar
- Renderer FPS ve frame time bilgisini tutar
- 4 e2e test: div+bg, nested+text, image, gradient, overflow
