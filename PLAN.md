# uwebr — Rust Native Desktop App Framework

> Next.js benzeri DX, %100 Rust, GPU ile çizim. Tarayıcı yok, HTML yok.

---

## 🎯 Vizyon

JavaScript/TypeScript/HTML/CSS kodunu alıp, doğrudan GPU ile çizilen masaüstü uygulamalarına çeviren bir ekosistem. Next.js'in geliştirici deneyimini (dosya tabanlı yönlendirme, component model, hot reload) Rust'a taşıyıp, tarayıcı yerine wgpu + vello ile ekrana çizen bir framework.

---

## 📁 Workspace Yapısı

```
uwebr/
├── Cargo.toml                      # Workspace root
├── crates/
│   ├── uwebr-js/                   # ✅ MEVCUT — JS/TS → Rust transpiler
│   ├── uwebr-html/                 # 🆕 HTML parser → rsx! component tree
│   ├── uwebr-css/                  # 🆕 CSS parser → Taffy layout styles
│   ├── uwebr-core/                 # 🆕 Framework: state, lifecycle, routing
│   ├── uwebr-render/               # 🆕 GPU rendering: vello + taffy + parley
│   ├── uwebr-app/                  # 🆕 Window management + event loop
│   └── uwebr-cli/                  # 🆕 `cargo uwebr create` scaffolding
├── examples/
│   ├── counter/
│   ├── todo/
│   └── dashboard/
└── docs/
    ├── ARCHITECTURE.md
    ├── PLAN.md                     # Bu dosya
    └── API_REFERENCE.md
```

---

## 🛠️ Teknoloji Stack'i (2026 En İyileri)

| Katman | Crate | Versiyon | Neden |
|--------|-------|----------|-------|
| Pencere Yönetimi | `winit` | 0.30.x | Tüm Rust GUI frameworklerinin ortak altyapısı, 6.1K stars |
| GPU Soyutlama | `wgpu` | 30.x | WebGPU standardı, Vulkan/Metal/DX12/WebGPU, 12.8M+ download |
| 2D Vektörel Çizim | `vello` | 0.10.0 | GPU compute-centric, 177 FPS (30K SVG benchmark) |
| CPU Fallback | `vello_cpu` | 0.0.6 | GPU yokken CPU ile çizim, sparse strips |
| Text Yerleşimi | `parley` | 0.9.0 | Linebender ekosistemi, inline box, bidirectional text |
| Text Rendering | `vello` (built-in) | - | Vector text rendering, glyphon alternatif |
| Layout Motoru | `taffy` | 0.14.0 | CSS Flexbox/Grid/Block, 100-1000x Yoga'dan hızlı |
| Biçimler | `kurbo` | - | Bezier eğrileri, vello entegrasyonu |
| HTML Parse | `swc_html` / `markup5ever` | - | HTML5 parsing |
| CSS Parse | `lightningcss` | - | Firefox CSS parser'ı, hızlı ve doğru |
| JS Parse (mevcut) | `swc_ecma_parser` | 45.1 | ES2020+ parsing, zaten kullanılıyor |
| Error Handling | `anyhow` + `thiserror` | - | Zaten mevcut |
| CLI | `clap` | 4.x | Zaten mevcut |

### Mimari Diagram

```
                    Uygulama Katmanı
                         │
                   ┌─────┴─────┐
                   │   State   │
                   │  Manager  │
                   │ (Signals) │
                   └─────┬─────┘
                         │
             ┌───────────┼───────────┐
             │           │           │
        ┌────┴────┐ ┌────┴────┐ ┌────┴────┐
        │ taffy   │ │ parley  │ │ Event   │
        │ Layout  │ │  Text   │ │ System  │
        │ Engine  │ │         │ │         │
        └────┬────┘ └────┬────┘ └────┬────┘
             │           │           │
             └───────────┼───────────┘
                         │
                   ┌─────┴─────┐
                   │   winit   │  Pencere + Event Loop
                   └─────┬─────┘
                         │
             ┌───────────┼───────────┐
             │                       │
       ┌─────┴─────┐          ┌─────┴─────┐
       │   wgpu    │          │ softbuffer│  (CPU fallback)
       │   (GPU)   │          │           │
       └─────┬─────┘          └─────┬─────┘
             │                       │
       ┌─────┴─────┐          ┌─────┴─────┐
       │   vello   │          │ vello_cpu │
       │  (GPU 2D) │          │  (CPU 2D) │
       └───────────┘          └───────────┘
```

### Veri Akışı

```
HTML/CSS/JS Dosyaları
        │
        ▼
┌──────────────────┐
│  Parse Katmanı   │  swc_html + lightningcss + swc_ecma_parser
│  (AST üretimi)   │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  Transform Katman│  uwebr-html: HTML → rsx! AST
│  (AST → AST)     │  uwebr-css: CSS → Taffy Style
│                  │  uwebr-js: JS → Rust AST (mevcut)
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  Codegen Katmanı │  rsx! macro, Rust fonksiyonları
│  (AST → Kod)     │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  Runtime         │  uwebr-core: State, Lifecycle, Diff
│  (Çalışma Zamanı)│  uwebr-render: Vello + Taffy + Parley
│                  │  uwebr-app: Winit EventLoop
└──────────────────┘
         │
         ▼
    GPU Ekranı
```

---

## 📦 Crate Detayları

### 1. uwebr-js (MEVCUT)

**Durum:** ✅ Tamamlandı (12/12 özellik, 13 test, 0 hata)

**Yetenekler:**
- JS/TS → Rust code transpilation
- Class → struct/enum/impl
- Arrow functions → closures
- Async/await → async fn
- for-of/for-in → for loop / iter
- String methods (10+)
- Object spread → HashMap::from_iter
- fetch/Promise patterns
- JSON.parse/stringify → serde_json
- Console.log/error/warn → println!/eprintln!
- Optional chaining → .as_ref().map()
- Nullish coalescing → .unwrap_or()
- Try/catch/throw → Result pattern
- Iterator methods → .iter() injection

### 2. uwebr-html (YENİ)

**Amaç:** HTML'i Rust component AST'sine çevir

**Girdi:**
```html
<div class="container">
  <h1>{title}</h1>
  {#each items as item}
    <p>{item.name}</p>
  {/each}
</div>
```

**Çıktı (rsx! formatı):**
```rust
rsx! {
    div(class: "container") {
        h1() { "{title}" }
        for item in items.iter() {
            p() { "{item.name}" }
        }
    }
}
```

**AST Tipleri:**
```rust
pub enum HtmlNode {
    Element(HtmlElement),
    Text(String),
    Expression(RsExpr),
    Component(HtmlComponent),
    EachLoop(HtmlEach),
    IfBlock(HtmlIf),
    Fragment(Vec<HtmlNode>),
}

pub struct HtmlElement {
    pub tag: String,
    pub attributes: Vec<HtmlAttribute>,
    pub children: Vec<HtmlNode>,
    pub self_closing: bool,
}

pub struct HtmlAttribute {
    pub name: String,
    pub value: HtmlAttributeValue,
}

pub enum HtmlAttributeValue {
    Literal(String),
    Expression(RsExpr),
    Boolean(bool),
    Shorthand(String),
}
```

**Desteklenecek Şablon Sözdizimi:**
- `{expression}` — ifade interpolasyonu
- `{#each items as item}...{/each}` — döngü
- `{#if condition}...{:else}...{/if}` — koşullu
- `<Component props />` — component composition
- `{@html raw_html}` — raw HTML insertion
- `on:click={handler}` — event handler

**Kullanılacak Kütüphane:** `markup5ever` (html5ever wrapper) veya `swc_html`

### 3. uwebr-css (YENİ)

**Amaç:** CSS'i Taffy `Style` objesine çevir

**Girdi:**
```css
.card {
    display: flex;
    flex-direction: column;
    padding: 16px;
    gap: 8px;
    background: linear-gradient(#333, #666);
    border-radius: 8px;
    box-shadow: 0 4px 6px rgba(0,0,0,0.1);
}

.card:hover {
    transform: translateY(-2px);
}
```

**Çıktı:**
```rust
fn card_style() -> Style {
    Style::default()
        .display(Display::Flex)
        .flex_direction(FlexDirection::Column)
        .padding(Val::Px(16.0))
        .gap(LengthPercentage::Length(Length::Px(8.0)))
        .border_radius(Val::Px(8.0))
        .box_shadow(BoxShadow {
            x_offset: 0.0,
            y_offset: 4.0,
            blur: 6.0,
            color: Color::rgba(0.0, 0.0, 0.0, 0.1),
        })
}
```

**CSS → Taffy Mapping:**

| CSS Property | Taffy Style |
|-------------|-------------|
| `display: flex` | `Display::Flex` |
| `flex-direction: column` | `FlexDirection::Column` |
| `justify-content: center` | `JustifyContent::Center` |
| `align-items: stretch` | `AlignItems::Stretch` |
| `padding: 16px` | `padding(Val::Px(16.0))` |
| `margin: auto` | `margin(Val::Auto)` |
| `width: 100%` | `width(Val::Percent(100.0))` |
| `grid-template-columns: 1fr 2fr` | Grid template |
| `position: absolute` | `position(PositionType::Absolute)` |

**Desteklenecek CSS Özellikleri:**
- Flexbox (tam destek)
- Grid (temel destek)
- Position (static, relative, absolute, fixed)
- Sizing (width, height, min/max)
- Spacing (margin, padding)
- Border (radius, width, color)
- Background (solid, gradient)
- Shadow (box-shadow)
- Opacity
- Transform (translate, scale, rotate)
- Media queries (temel)

**Kullanılacak Kütüphane:** `lightningcss` (Firefox CSS parser'ı)

### 4. uwebr-core (YENİ)

**Amaç:** Framework çekirdeği — React/Leptos benzeri state management

**Signals (Reactive State):**
```rust
// Sinyal oluştur
let (count, set_count) = create_signal(0);

// Memo (hesaplanmış değer)
let doubled = create_memo(move || *count() * 2);

// Effect (yan etki)
create_effect(move |_| {
    println!("Count changed: {}", count());
});

// Setter güncelleme
set_count.update(|c| *c += 1);
```

**Component Model:**
```rust
#[component]
fn Counter(initial: i32) -> Element {
    let (count, set_count) = create_signal(initial);

    rsx! {
        div {
            span { "Count: {count}" }
            button(on:click = move |_| set_count.update(|c| *c += 1)) {
                "+1"
            }
        }
    }
}
```

**Lifecycle:**
```rust
#[component]
fn DataFetcher(url: String) -> Element {
    let (data, set_data) = create_signal(None);

    on_mount(move || {
        // Async veri çekme
        spawn(async move {
            let result = fetch(&url).await;
            set_data.set(Some(result));
        });
    });

    on_cleanup(|| {
        // Temizlik
    });

    rsx! {
        match data() {
            Some(d) => rsx! { div { "{d}" } },
            None => rsx! { div { "Loading..." } },
        }
    }
}
```

**Context (Global State):**
```rust
let theme = provide_context(create_signal("dark"));

// Herhangi bir child component'te
let theme = use_context::<Signal<String>>();
```

**Props:**
```rust
#[derive(Props)]
struct CardProps {
    title: String,
    #[props(default = false)]
    highlighted: bool,
    children: Element,
}

#[component]
fn Card(props: CardProps) -> Element {
    rsx! {
        div(class: if props.highlighted { "card active" } else { "card" }) {
            h2 { "{props.title}" }
            {props.children}
        }
    }
}
```

### 5. uwebr-render (YENİ)

**Amaç:** Component tree → GPU draw calls

**Pipeline:**
```
Component Tree (rsx! output)
        │
        ▼
Virtual DOM Diff (minimal patch hesaplama)
        │
        ▼
Layout Tree → taffy.compute_layout()
        │
        ▼
Render Commands → vello::Scene building
        │
        ▼
wgpu Surface → Ekranda piksel
```

**Render Node Tipleri:**
```rust
pub enum RenderCommand {
    Rect { rect: Rect, fill: Brush, stroke: Option<Stroke> },
    RoundRect { rect: Rect, radius: f32, fill: Brush, stroke: Option<Stroke> },
    Text { text: String, font_size: f32, color: Color, bounds: Rect },
    Image { image_id: ImageId, rect: Rect },
    Clip { rect: Rect, children: Vec<RenderCommand> },
    Transform { matrix: Affine, children: Vec<RenderCommand> },
    PushLayer { opacity: f32, children: Vec<RenderCommand> },
}
```

**Hit Testing (Mouse Event Mapping):**
```rust
fn hit_test(&self, point: Point) -> Option<NodeId> {
    // Tree traversal with taffy layout bounds
    // Returns deepest node that contains the point
}
```

**Text Rendering (Parley + Vello):**
```rust
fn render_text(&self, scene: &mut Scene, text: &str, bounds: Rect, style: &TextStyle) {
    let mut layout_context = parley::LayoutContext::new();
    let mut buffer = layout_context.layout(text, style);
    // Vello vector text rendering
}
```

**Desteklenecek Görsel Özellikler:**
- Solid fill, linear/radial gradient
- Border (width, color, radius)
- Box shadow
- Opacity
- Clip (overflow: hidden)
- Transform (translate, scale, rotate)
- Scroll (overflow: scroll)
- Text (font-size, font-weight, color, alignment)
- Image (PNG, JPEG, SVG)

### 6. uwebr-app (YENİ)

**Amaç:** Winit event loop entegrasyonu

```rust
pub struct UwebrApp {
    window: Window,
    renderer: Renderer,          // uwebr-render
    root_component: ComponentFn,
    state: AppState,             // uwebr-core
    layout_cache: LayoutCache,   // taffy output cache
}

impl ApplicationHandler for UwebrApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // GPU device initialization
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => {
                // 1. State changes → re-render component tree
                // 2. Virtual DOM diff
                // 3. Taffy layout computation
                // 4. Vello scene building
                // 5. wgpu render
            }
            WindowEvent::CursorMoved { position, .. } => {
                // Hit test → mouse event dispatch
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // Focus management → keyboard event dispatch
            }
            _ => {}
        }
    }
}
```

**Multi-Window:**
```rust
UwebrApp::new()
    .window("main", WindowConfig { title: "My App", width: 800, height: 600 })
    .window("settings", WindowConfig { title: "Settings", width: 400, height: 300 })
    .run();
```

**Timer/Animation:**
```rust
// Her 16ms'de bir (60 FPS)
request_animation_frame(|| {
    // Frame update
});

// 1 saniye sonra
set_timeout(|| {
    // Delayed action
}, Duration::from_secs(1));
```

### 7. uwebr-cli (YENİ)

**Amaç:** Developer experience — proje oluşturma ve geliştirme

```bash
# Yeni proje oluştur
cargo uwebr create my-app
# → src/pages/index.rs (anasayfa component)
# → src/pages/about.rs (hakkında component)
# → src/styles/main.css
# → src/main.rs (app entry point)
# → Cargo.toml (dependencies)

# Geliştirme modu (hot reload)
cargo uwebr dev
# → Dosya değişikliği algılama
# → Incremental rebuild
# → Pencere otomatik refresh

# Production build
cargo uwebr build --release
# → Optimize edilmiş binary
# → Asset embedding (images, fonts)

# Preview
cargo uwebr preview
# → Test render without full build
```

**Dosya Tabanlı Routing (Next.js benzeri):**
```
src/pages/
├── index.rs          → "/"
├── about.rs          → "/about"
├── blog/
│   ├── index.rs      → "/blog"
│   └── [slug].rs     → "/blog/:slug" (dynamic)
├── dashboard/
│   ├── index.rs      → "/dashboard"
│   └── settings.rs   → "/dashboard/settings"
└── 404.rs            → fallback
```

**CLI Komutları:**
| Komut | Açıklama |
|-------|---------|
| `cargo uwebr create <name>` | Yeni proje oluştur |
| `cargo uwebr dev` | Hot reload ile geliştirme |
| `cargo uwebr build` | Production binary |
| `cargo uwebr preview` | Hızlı test render |
| `cargo uwebr component <name>` | Yeni component oluştur |
| `cargo uwebr page <path>` | Yeni sayfa oluştur |

---

## 🚀 Fazlara Ayrılmış Yol Haritası

### FAZ 0 — Workspace Kurulumu
**Süre:** 1-2 saat
**Hedef:** Mevcut uwebr-js'i workspace yapısına taşı

- [ ] Root `Cargo.toml` oluştur (workspace)
- [ ] `uwebr-js`'i `crates/uwebr-js` altına taşı
- [ ] Ortak dependency versiyonlarını paylaş
- [ ] Workspace build test et
- [ ] README.md güncelle

### FAZ 1 — uwebr-html
**Süre:** 1-2 hafta
**Hedef:** HTML → rsx! component AST

- [ ] `crates/uwebr-html` oluştur
- [ ] HTML parser (markup5ever veya swc_html)
- [ ] AST tanımları (HtmlNode, HtmlElement, HtmlAttribute)
- [ ] rsx! formatında codegen
- [ ] `{expression}` interpolasyon
- [ ] `{#each}` loop desteği
- [ ] `{#if}` conditional desteği
- [ ] Component composition (`<Component />`)
- [ ] Integration tests (10+ test)

### FAZ 2 — uwebr-css
**Süre:** 1-2 hafta
**Hedef:** CSS → Taffy Style

- [ ] `crates/uwebr-css` oluştur
- [ ] CSS parser (lightningcss)
- [ ] CSS property → Taffy Style mapping
- [ ] Flexbox desteği (tam)
- [ ] Grid desteği (temel)
- [ ] Position desteği
- [ ] Sizing/Spacing desteği
- [ ] Border/Background/Shadow
- [ ] CSS variables → Rust const
- [ ] Media query desteği (temel)
- [ ] Integration tests (15+ test)

### FAZ 3 — uwebr-core
**Süre:** 2-3 hafta
**Hedef:** State management + lifecycle

- [ ] `crates/uwebr-core` oluştur
- [ ] Signal sistemi (create_signal, create_memo, create_effect)
- [ ] Component model (#[component] macro)
- [ ] Props sistemi (#[derive(Props)])
- [ ] Lifecycle hooks (on_mount, on_cleanup)
- [ ] Context (provide_context, use_context)
- [ ] Virtual DOM diffing
- [ ] Event system (on:click, on:input)
- [ ] Spawn/async desteği
- [ ] Integration tests (10+ test)

### FAZ 4 — uwebr-render
**Süre:** 3-4 hafta
**Hedef:** GPU rendering pipeline

- [ ] `crates/uwebr-render` oluştur
- [ ] Vello scene builder
- [ ] Rect/RoundRect/Line çizimi
- [ ] Text rendering (parley + vello)
- [ ] Gradient desteği
- [ ] Shadow desteği
- [ ] Opacity/clip desteği
- [ ] Transform (translate, scale, rotate)
- [ ] Image rendering
- [ ] Hit testing
- [ ] Scroll container
- [ ] Taffy layout integration
- [ ] Benchmarks

### FAZ 5 — uwebr-app
**Süre:** 2 hafta
**Hedef:** Pencere + event loop

- [ ] `crates/uwebr-app` oluştur
- [ ] Winit ApplicationHandler
- [ ] GPU device initialization
- [ ] RedrawRequested → render pipeline
- [ ] Mouse event dispatch (hit test)
- [ ] Keyboard event dispatch
- [ ] Multi-window desteği
- [ ] Timer/animation frame
- [ ] File dialog
- [ ] Clipboard

### FAZ 6 — uwebr-cli
**Süre:** 1 hafta
**Hedef:** Developer experience

- [ ] `crates/uwebr-cli` oluştur
- [ ] `create` komutu (scaffolding)
- [ ] `dev` komutu (hot reload)
- [ ] `build` komutu (production)
- [ ] `component` komutu (yeni component)
- [ ] `page` komutu (yeni sayfa)
- [ ] Dosya değişikliği algılama (notify crate)
- [ ] Incremental rebuild

---

## 📊 Zaman Çizelgesi

| Faz | Açıklama | Süre | Toplam |
|-----|---------|------|--------|
| 0 | Workspace kurulumu | 1-2 saat | 1-2 saat |
| 1 | uwebr-html | 1-2 hafta | 1-2 hafta |
| 2 | uwebr-css | 1-2 hafta | 2-4 hafta |
| 3 | uwebr-core | 2-3 hafta | 4-7 hafta |
| 4 | uwebr-render | 3-4 hafta | 7-11 hafta |
| 5 | uwebr-app | 2 hafta | 9-13 hafta |
| 6 | uwebr-cli | 1 hafta | 10-14 hafta |

**MVP (statik sayfa GPU'da):** ~6-8 hafta
**Full framework (Next.js seviyesi):** ~3-4 ay

---

## 🎯 Kullanıcı Deneyimi Örneği

```rust
// pages/counter.rs — Next.js gibi dosya tabanlı routing
use uwebr::prelude::*;

#[component]
fn Counter() -> Element {
    let (count, set_count) = create_signal(0);

    rsx! {
        div(class: "flex flex-col items-center p-8") {
            h1(class: "text-2xl font-bold") {
                "Counter: {count}"
            }
            button(
                class: "bg-blue-500 text-white px-4 py-2 rounded",
                on:click = move |_| set_count.update(|c| *c += 1)
            ) {
                "+1"
            }
            button(
                class: "bg-red-500 text-white px-4 py-2 rounded",
                on:click = move |_| set_count.update(|c| *c -= 1)
            ) {
                "-1"
            }
        }
    }
}

// pages/dashboard.rs — nested routing
use uwebr::prelude::*;

#[component]
fn Dashboard() -> Element {
    let (items, set_items) = create_signal(vec![
        Item { id: 1, name: "First".into() },
        Item { id: 2, name: "Second".into() },
    ]);

    rsx! {
        div(class: "p-4") {
            h1(class: "text-xl") { "Dashboard" }
            for item in items().iter() {
                Card(title: item.name.clone()) {
                    p { "ID: {item.id}" }
                }
            }
        }
    }
}

// main.rs — otomatik dosya tabanlı routing
fn main() {
    UwebrApp::new()
        .route("/", Counter)
        .route("/dashboard", Dashboard)
        .title("My App")
        .run();  // wgpu + winit ile GPU'ya çizilir
}
```

---

## 📈 Mevcut Durum

| Component | Durum | Test |
|-----------|-------|------|
| uwebr-js | ✅ Tamamlandı | 13/13 |
| uwebr-html | ❓ Planlandı | - |
| uwebr-css | ❓ Planlandı | - |
| uwebr-core | ❓ Planlandı | - |
| uwebr-render | ❓ Planlandı | - |
| uwebr-app | ❓ Planlandı | - |
| uwebr-cli | ❓ Planlandı | - |

---

## 🔗 Referanslar

- [Taffy](https://github.com/DioxusLabs/taffy) — CSS Flexbox/Grid layout engine
- [Vello](https://github.com/linebender/vello) — GPU compute 2D renderer
- [Parley](https://github.com/linebender/parley) — Text layout
- [Wgpu](https://github.com/gfx-rs/wgpu) — WebGPU abstraction
- [Winit](https://github.com/rust-windowing/winit) — Window management
- [LightningCSS](https://github.com/parcel-bundler/lightningcss) — CSS parser
- [Leptos](https://github.com/leptos-rs/leptos) — Reference for signals/component model
- [Slint](https://slint.dev/) — Reference for declarative UI
- [Xilem](https://github.com/linebender/xilem) — Reference for reactive UI with vello

---

*Son güncelleme: Ağustos 2026*
