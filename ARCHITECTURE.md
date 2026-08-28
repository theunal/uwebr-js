# uwebr — Mimari Rehber

## Genel Bakış

uwebr, `.uwebr` dosyalarını (HTML + `<script>` + `<style>`) alıp GPU ile çizilen masaüstü uygulamalarına çeviren bir Rust frameworküdür. Next.js benzeri bir geliştirici deneyimi sunar; tarayıcı yerine wgpu + vello ile doğrudan GPU'ya çizer.

## Temel Prensipler

1. **Tarayıcı Yok**: HTML/CSS/JS → Rust transformasyonu, tarayıcı DOM'u yok
2. **GPU-First**: Tüm çizimler wgpu + vello ile GPU'da yapılır
3. **Component-Based**: React/Leptos benzeri component model
4. **Reactive State**: Fine-grained signals (Leptos benzeri)
5. **Cross-Platform**: Windows, macOS, Linux (aynı kod tabanı)

## Workspace Yapısı (8 Crate)

```
uwebr/
├── uwebr-js/      # JS/TS → Rust transpiler (swc_ecma_parser) + script state lowering
├── uwebr-html/    # HTML parser (html5ever) + template directives
├── uwebr-css/     # Custom CSS parser → Taffy Style + PaintProps
├── uwebr-core/    # Reactive system: Signal, Effect, Memo, Context, Timer, script state
├── uwebr-macro/   # #[component] + #[derive(Props)] proc macros
├── uwebr-render/  # Layout (Taffy) + Scene (vello) + Text (Parley) + StyleBook + Paint
├── uwebr-app/     # Winit + wgpu + vello App, Multi-window, RenderPipeline, GpuContext
└── uwebr-cli/     # CLI: init/build/check/dev + transpiler
```

**Crate sınırı notu:** `uwebr-render` GPU cihazı/surface'i tutmaz — yalnız `vello::Scene` üretir. wgpu device, surface ve blit işi `uwebr-app::GpuContext`'te. Bu nedenle `uwebr-render` wgpu/winit'e doğrudan bağlı değildir.

## Katmanlı Mimari

```
┌─────────────────────────────────────────────────────────┐
│                    Kullanıcı Kodu                       │
│  .uwebr dosyaları (HTML + <script> + <style>)           │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                  Transpile Katmanı                      │
│  uwebr-html:  HTML → Element AST (html5ever)            │
│  uwebr-css:   CSS → (Taffy Style, StyleMask, Paint)     │
│  uwebr-js:    JS → Rust AST (swc_ecma_parser)           │
│               + top-level let → reaktif accessor        │
│  uwebr-cli:   .uwebr → Rust .rs codegen                 │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                 Framework Katmanı                       │
│  uwebr-core: Signals, Effects, Lifecycle, Timer         │
│              state (keyed script state), actions        │
│  uwebr-macro: #[component], #[derive(Props)]            │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                 Rendering Katmanı                       │
│  uwebr-render:                                          │
│    StyleBook:  CSS rules → tag/class/id matching        │
│    Paint:      ResolvedPaint (renk, font, kalıtım)      │
│    Layout:     Element → TaffyTree → PositionedNode     │
│                (compute_layout_with_measure → parley)   │
│    Text:       Parley font layout + measure             │
│    Scene:      PositionedNode → vello::Scene            │
│                (fill + draw_glyphs)                     │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                 Platform Katmanı                        │
│  uwebr-app:                                             │
│    GpuContext:  wgpu device + surface + vello renderer  │
│                 + Rgba8Unorm storage texture + blit     │
│    App:         Winit ApplicationHandler                │
│    Multi-window: HashMap<WindowId, WindowState>         │
│    Pipeline:    hit-test (on:click) + scene üretimi     │
└─────────────────────────────────────────────────────────┘
```

## Veri Akışı (Pipeline)

```
.uwebr Dosyası
  ├── <div class="box">Hello {count}</div>
  ├── <style>.box { width: 200px; color: #e0e0e0; }</style>
  └── <script>let count = 0;</script>
        │
        ▼
┌──────────────────────────┐
│ 1. PARSE (uwebr-html)    │  html5ever → Element AST
│    parse_html(content)   │  {#each}, {#if}, <Component/>
└────────────┬─────────────┘
             │
             ▼
┌──────────────────────────┐
│ 2. CSS MATCH (StyleBook) │  StyleBook::parse(css)
│    match_full(el)        │  → MatchedStyle { style, mask, paint }
│                          │  tag < class < id, yalnız set edilmiş alanlar
└────────────┬─────────────┘
             │
             ▼
┌──────────────────────────┐
│ 3. LAYOUT (Taffy 0.14)   │  LayoutEngine::build_tree()
│    compute_layout_       │  Text node'lar NodeContext::Text taşır,
│      with_measure()      │  ölçüm parley'den gelir
│                          │  → Vec<PositionedNode> (absolute x/y + paint)
└────────────┬─────────────┘
             │
             ▼
┌──────────────────────────┐
│ 4. SCENE (vello)         │  SceneBuilder::build()
│    fill() / draw_glyphs()│  → vello::Scene
└────────────┬─────────────┘
             │
             ▼
┌──────────────────────────┐
│ 5. RENDER (wgpu+vello)   │  GpuContext::render_scene()
│    compute → storage tex │  Rgba8Unorm storage texture
│    blit → surface        │  → TextureBlitter → Surface → Ekran
└──────────────────────────┘
```

### Neden ara texture var?

Vello, sahneyi bir compute shader ile çizer ve **storage texture** bekler. Surface texture'ı çoğu platformda `RENDER_ATTACHMENT`-only ve sRGB'dir; doğrudan çizmeye kalkışmak runtime'da şu hatayı verir:

```
Storage texture binding 5 expects format = Rgba8Unorm,
but given a view with format = Bgra8UnormSrgb
```

Bu nedenle `GpuContext` bir `Rgba8Unorm` + `STORAGE_BINDING | TEXTURE_BINDING` ara texture tutar, vello oraya çizer, ardından `wgpu::util::TextureBlitter` ile surface'e blit edilir. Surface formatı da sRGB olmayan bir varyant (`Rgba8Unorm`/`Bgra8Unorm`) olarak seçilir; aksi halde transfer fonksiyonu iki kez uygulanır.

## Reactive System (uwebr-core)

```
create_signal(value) ──→ Signal<T> (read/write)
       │
       ├── signal.get()      → değeri oku (etkin effect'e abone olur)
       ├── setter.set(v)     → yeni değer + render dirty
       └── setter.update()   → functional update + render dirty

create_memo(compute) ──→ Memo<T> (derived, lazy)
create_effect(closure) ──→ Side effect (deps değiştiğinde çalışır)

Render dirty bayrağı:
       │
       ├── mark_render_dirty()  → repaint gerekiyor
       ├── is_render_dirty()    → bayrağı oku
       └── take_render_dirty()  → oku + temizle (event loop kullanır)

Script state (uwebr_core::state):
       │
       ├── get(key, initial)  → sinyali oku (yoksa oluştur)
       ├── set(key, value)    → sinyale yaz (+ render dirty)
       └── clear()            → tüm script state'i sıfırla

Named actions (uwebr_core::events):
       │
       ├── register_action(name, handler)  → component her render'da çağırır
       ├── dispatch_action(name)           → tıklamada çağrılır
       └── has_action(name) / clear_actions()

Timer System (global OnceLock<TimerRegistry>):
       │
       ├── set_timeout / set_interval / request_animation_frame
       └── cancel_timer(handle)
```

### Repaint tetikleyicileri

`App::about_to_wait` iki kaynağı birlikte kontrol eder:

1. `timer_registry().has_pending()` — bekleyen timeout/interval/animation frame
2. `take_render_dirty()` — herhangi bir signal ya da script state yazımı

İkisinden biri doğruysa tüm pencerelere `request_redraw()` gönderilir.

## Component Model

```rust
// .uwebr dosyasından transpile edilen kod:
pub const CSS_APP: &str = r#".app { ... }"#;

fn __state_count() -> i64 {
    return uwebr_core::state::get("count".to_string(), 0);
}
fn __set_state_count(value: i64) {
    uwebr_core::state::set("count".to_string(), value);
}
fn increment() {
    __set_state_count(__state_count() + 1);
}

pub fn app_component() -> Element {
    uwebr_core::events::register_action("increment", increment);
    Element {
        node_type: NodeType::Element("div".into()),
        props: vec![("class".into(), PropValue::String("app".into()))],
        children: vec![
            Element {
                node_type: NodeType::Element("button".into()),
                props: vec![("on:click".into(), PropValue::Closure("increment".into()))],
                children: vec![Element::text("+")],
            },
            Element { node_type: NodeType::Text((__state_count()).to_string()), .. },
        ],
    }
}
```

Ana uygulama (`src/main.rs`, `uwebr build` tarafından üretilir):

```rust
fn main() -> anyhow::Result<()> {
    let mut app = App::new("App");
    app = app.with_css(CSS_APP);
    app.with_component(FnComponent::new(|| app_component()))
        .run()
}
```

### `<script>` state lowering

Top-level `let count = 0;` doğrudan yazılamaz — modül kapsamında `let` Rust'ta geçersizdir ve fonksiyonlardan erişilemez. `uwebr_js::script::lower_script_state` her top-level binding'i bir getter/setter çiftine indirger; okumalar sinyale abone olur, yazmalar repaint tetikler.

### `on:click` yolu

```
on:click={increment}
    │  transpiler
    ▼
PropValue::Closure("increment")  +  register_action("increment", increment)
    │  layout
    ▼
RenderPipeline::hit_targets — (action, absolute bounds, depth)
    │  winit MouseInput
    ▼
pipeline.hit_test(x, y) → en derin hedef → dispatch_action(name)
    │  handler signal'a yazar
    ▼
take_render_dirty() → request_redraw()
```

## Multi-Window

```rust
App::new("Main")
    .with_component(MainComponent)
    .open_window("Settings", 400, 300, SettingsComponent)
    .run()?;

// App internally:
// HashMap<WindowId, WindowState> — per-window GPU + pipeline + component + cursor
// pending_windows: Vec<(String, w, h, Box<dyn Component>)> — queue for creation
```

## Layout Hesaplama (Taffy 0.14)

```
Element tree
    │
    ▼
StyleBook::match_full(el) → MatchedStyle { style, mask, paint }
    │
    ├── mask: kuralda gerçekten belirtilmiş property'ler
    │         (yalnız bunlar yazılır — cascade doğru çalışsın)
    ├── Tag defaults: yalnız mask'ta olmayan alanlara uygulanır
    └── Inline props: class="box" → width: 200px
    │
    ▼
Text node → TaffyTree::new_leaf_with_context(NodeContext::Text { content, font_size, .. })
Diğerleri → TaffyTree::new_with_children()
    │
    ▼
taffy.compute_layout_with_measure(root, available_space, measure_fn)
    │  measure_fn → TextRenderer::measure (parley) → (w, h)
    ▼
Vec<PositionedNode> — absolute x, y, width, height + ResolvedPaint
```

Kök element viewport'u kaplar (`width/height: 100%`, CSS aksini söylemedikçe): `align-items: center` gibi kurallar ancak böyle hizalanacak bir alan bulur.

### Paint kalıtımı

Taffy yalnız yerleşim bilir, dolayısıyla `background-color` / `color` / `font-size` layout sınırında düşerdi. `ResolvedPaint` bunları taşır:

- `color`, `font_size`, `font_family` → çocuklara kalıtılır (metin ancak böyle renklenir)
- `background`, `border`, `opacity` → kalıtılmaz (CSS semantiği)
- Öncelik: kalıtılan < CSS kuralı < inline prop

## Rendering Pipeline (Vello 0.10)

```
PositionedNode[]
    │
    ▼
SceneBuilder::build():
  surface arka planı (siyah fill)
  for node in positioned_nodes:
    match node.kind:
      Container → background varsa fill(Rect | RoundedRect, brush)
      Text      → parley Layout → line.items() → GlyphRun
                  → scene.draw_glyphs(font).brush(color).draw(glyphs)
      Rect      → fill(Rect, brush)
      RoundRect → fill(RoundedRect, brush)
    border varsa → stroke()
    overflow_hidden ise → push_clip_layer / pop_layer
    opacity < 1 ise     → push_layer / pop_layer
    │
    ▼
GpuContext::render_scene():
  vello_renderer.render_to_texture(&device, &queue, scene, &target_view, params)
  blitter.copy(&device, &mut encoder, &target_view, &surface_view)
  queue.submit + surface_texture.present()
    │
    ▼
Ekran (AutoVsync)
```

Fontsuz ortamlar (headless CI, minimal image) için `TextRenderer::measure` bir tahmine düşer: parley 0 boyut döndürürse karakter sayısı × font boyutu oranıyla ölçü üretilir. Aksi halde metin node'u 0×0 hesaplanır ve sahneden düşer.

## CLI Pipeline

```
uwebr init my-app
  ├── Scaffold: Cargo.toml, src/app/App.uwebr, src/components/, public/
  ├── Bağımlılıklar: mümkünse uwebr checkout'una path dependency
  └── İlk transpile: src/generated/{app.rs, mod.rs} + src/main.rs
      (bu adım olmadan önerilen `cargo run` derlenmezdi)

uwebr build [--release]
  └── .uwebr → transpiler::transpile() → .rs files
      ├── src/generated/app.rs   (pub const CSS_*, state accessors, component fn)
      ├── src/generated/mod.rs   (snake_case modül adları)
      ├── src/main.rs            (App + with_css + FnComponent)
      └── cargo build [--release]

uwebr check
  └── Parse all .uwebr files (validate only, no transpile)

uwebr dev
  └── File watcher (notify 7) + 100ms debounce
      ├── Initial: BuildCache::build_all() (parse tanılama)
      │            + transpile_all() + cargo build + uygulamayı başlat
      ├── On change: BuildCache::build_incremental(changed)
      │            + transpile_incremental() + cargo build
      │            + eski süreci kill/wait → yeniden spawn
      └── Build hatasında çalışan uygulama ayakta kalır
```

**Windows notu:** uygulama, build çıktısının bir kopyasından (`<name>-dev-run.exe`) çalıştırılır. Çalışan bir çalıştırılabilir dosya kilitlendiği için, doğrudan `target/debug/app.exe` başlatılsa sonraki `cargo build` link aşamasında başarısız olur ve bu, gerçek bir derleme hatasından ayırt edilemezdi.

## Performance Hedefleri

| Metrik | Hedef | Ölçülen | Notlar |
|--------|-------|---------|--------|
| İlk render | < 100ms | ölçülmedi | Cold start |
| Frame rate | 60 FPS | ölçülmedi | AutoVsync |
| Memory | < 50MB | ölçülmedi | Typical desktop app |
| Binary size | < 10MB | ölçülmedi | Optimized release |
| Hot reload | < 500ms | **~7s** | `cargo build` süresi baskın; hedef gerçekçi değil |
| Layout compute | < 1ms | ölçülmedi | 1000 node tree |

Hot reload ölçümü: dosya kaydından yeni sürecin ayağa kalkmasına kadar 6.9 s (debug profili, tek `.uwebr` dosyalı scaffold). Bunun neredeyse tamamı `cargo build`; transpile adımı ~60 ms. `< 500 ms` hedefi ancak süreç yeniden başlatmadan (in-process reload) mümkün olur.

## Doğrulama Durumu

| İddia | Nasıl doğrulandı |
|-------|------------------|
| Metin ekrana çıkıyor | `cargo run -p uwebr-app --example gpu_probe` → 17 glyph, 947 px `#e0e0e0` |
| CSS arka planı çıkıyor | aynı probe → yüzeyin baskın rengi `#1a1a2e` |
| Scaffold derleniyor | `uwebr init` + `cargo build` → başarılı |
| Uygulama açılıyor | üretilen binary 7 s boyunca çalıştı, panik yok |
| Hot reload çalışıyor | `uwebr dev` + dosya değişimi → yeni PID, 6.9 s |
| Tıklama → state → yeniden render | `cargo test -p uwebr-app --test interaction_tests` (8 test) |
| Test sayısı | `cargo test --workspace` → 444 test |

Ölçülmeyenler: FPS, bellek, binary boyutu, ilk render süresi, 1000 node layout süresi.

### Tanılama örnekleri

Bu tablodaki iddiaları yeniden üretmek için:

```bash
cargo run -p uwebr-app --example gpu_probe         # headless GPU render + framebuffer analizi
cargo run -p uwebr-render --example glyph_probe    # glyph üretimi + metin ölçümü
cargo run -p uwebr-render --example layout_probe   # font-size → text box yüksekliği
cargo run -p uwebr-cli --example scaffold_output   # scaffold'ın ürettiği Rust
```

## Bilinen Sınırlar

- **Component props callee'ye geçirilmiyor** — `<Card title="x" />` prop'u `Element.props`'a yazılıyor, `card_component()` argüman almıyor.
- **`{@html expr}`** sahneye çıkmıyor; geçersiz Rust üretmiyor, sessizce düşüyor.
- **`RenderStyle::overflow_hidden`** CSS'ten doldurulmuyor. Sahne tarafı kırpmayı destekliyor (`push_clip_layer`), `pipeline.rs::paint_to_render_style` her zaman `false` yazıyor.
- **Gradient** CSS'ten gelmiyor — `Background::LinearGradient` desteği var, `uwebr-css` `linear-gradient(...)`'ı `Keyword` olarak saklıyor.
- **Pseudo-class / attribute selector'lar** parse ediliyor ama eşleşmede yok sayılıyor.
- **`vw`/`vh`** yüzde olarak yaklaşılıyor: kökte viewport'a, iç içe elementlerde ebeveyne göre çözülür.
- **`RenderNodeKind::Image`** gerçek görsel çizmiyor, `Rect` olarak düşüyor.
- **Metin kırpma/eliding yok** — taşan metin kutu dışına çizilir.

## Güvenlik

- **Memory safety**: Rust'ın guarantee'ları
- **No null pointer**: Option<T> kullanımı
- **No data races**: Send + Sync bounds
- **Sandboxed rendering**: GPU pipeline isolate
- **Input validation**: XSS-style attacks yok (tarayıcı DOM'u yok)
- **Codegen escaping**: attribute ve metin değerleri Rust string literal'i için escape edilir; aksi halde `title="say &quot;hi&quot;"` üretilen kodu bozardı

## İlgili Belgeler

| Belge | İçerik |
|-------|--------|
| `PLAN.md` | Yol haritası, faz durumları, test sayıları, bilinen sınırlar |
| `faz4.plan.md` | FAZ 4 (`uwebr-render`) planı ve plandan sapmalar |
| `faz8.plan.md` | FAZ 8 raporu: bulgu doğrulama tablosu, ölçümler, açık maddeler |
| `crates/uwebr-js/GAPS_PLAN.md` | uwebr-js durumu + script state lowering ayrıntısı |

---

*Son güncelleme: 28 Ağustos 2026 (FAZ 8)*
