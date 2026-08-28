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
├── uwebr-js/      # JS/TS → Rust transpiler (swc_ecma_parser)
├── uwebr-html/    # HTML parser (html5ever) + template directives
├── uwebr-css/     # Custom CSS parser → Taffy Style
├── uwebr-core/    # Reactive system: Signal, Effect, Memo, Context, Timer
├── uwebr-macro/   # #[component] + #[derive(Props)] proc macros
├── uwebr-render/  # Layout (Taffy) + Scene (vello) + Text (Parley) + StyleBook
├── uwebr-app/     # Winit + wgpu + vello App, Multi-window, RenderPipeline
└── uwebr-cli/     # CLI: init/build/check/dev + transpiler
```

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
│  uwebr-css:   CSS → Taffy Style (custom parser)         │
│  uwebr-js:    JS → Rust AST (swc_ecma_parser)           │
│  uwebr-cli:   .uwebr → Rust .rs codegen                 │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                 Framework Katmanı                       │
│  uwebr-core: Signals, Effects, Lifecycle, Timer         │
│  uwebr-macro: #[component], #[derive(Props)]            │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                 Rendering Katmanı                       │
│  uwebr-render:                                          │
│    StyleBook:  CSS rules → tag/class/id matching        │
│    Layout:     Element → TaffyTree → PositionedNode     │
│    Scene:      PositionedNode → vello::Scene            │
│    Text:       Parley font layout + measure             │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                 Platform Katmanı                        │
│  uwebr-app:                                             │
│    GpuContext:  wgpu device + surface + vello renderer  │
│    App:         Winit ApplicationHandler                │
│    Multi-window: HashMap<WindowId, WindowState>         │
│    Window:      per-window GPU context + pipeline       │
└─────────────────────────────────────────────────────────┘
```

## Veri Akışı (Pipeline)

```
.uwebr Dosyası
  ├── <div class="box">Hello</div>
  ├── <style>.box { width: 200px; }</style>
  └── <script>let x = 1;</script>
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
│    match_element(el)     │  → (Style, bool) by tag<class<id
└────────────┬─────────────┘
             │
             ▼
┌──────────────────────────┐
│ 3. LAYOUT (Taffy 0.14)   │  LayoutEngine::build_tree()
│    compute_layout()      │  → Vec<PositionedNode>
└────────────┬─────────────┘
             │
             ▼
┌──────────────────────────┐
│ 4. SCENE (vello)         │  SceneBuilder::build_scene()
│    fill(), stroke()      │  → vello::Scene
└────────────┬─────────────┘
             │
             ▼
┌──────────────────────────┐
│ 5. RENDER (wgpu+vello)   │  Renderer::render_frame()
│    GPU submit            │  → Surface texture → Screen
└──────────────────────────┘
```

## Reactive System (uwebr-core)

```
create_signal(value) ──→ Signal<T> (read/write)
       │
       ├── signal()        → değeri oku
       ├── set_signal(v)   → yeni değer ayarla
       └── signal.update() → functional update

create_memo(compute) ──→ Memo<T> (derived, lazy)
       │
       ├── Input sinyalleri değiştiğinde yeniden hesaplar
       └── Memoized: aynı input → aynı output

create_effect(closure) ──→ Side effect (deps değiştiğinde çalışır)
       │
       ├── Mount'ta bir kez çalışır
       ├── Read ettiği sinyaller değiştiğinde tekrar çalışır
       └── Cleanup: re-run/destroy'da temizleme

Timer System (global OnceLock<TimerRegistry>):
       │
       ├── set_timeout(closure, duration)     → TimerHandle
       ├── set_interval(closure, duration)    → TimerHandle
       ├── request_animation_frame(closure)   → TimerHandle
       └── cancel_timer(handle)               → bool
```

## Component Model

```rust
// .uwebr dosyasından transpile edilen kod:
pub fn my_component() -> Element {
    Element {
        node_type: NodeType::Element("div".into()),
        props: vec![("class".into(), PropValue::String("app".into()))],
        children: vec![
            Element { node_type: NodeType::Text("Hello".into()), .. },
        ],
    }
}

// Ana uygulama:
fn main() -> anyhow::Result<()> {
    App::new("My App")
        .with_size(800, 600)
        .with_css(".app { display: flex; }")
        .with_component(FnComponent::new(|| my_component()))
        .run()
}
```

## Multi-Window

```rust
App::new("Main")
    .with_component(MainComponent)
    .open_window("Settings", 400, 300, SettingsComponent)
    .run()?;

// App internally:
// HashMap<WindowId, WindowState> — per-window GPU + pipeline + component
// pending_windows: Vec<(String, w, h, Box<dyn Component>)> — queue for creation
```

## Layout Hesaplama (Taffy 0.14)

```
Element tree
    │
    ▼
element_to_style(el, stylebook)  → taffy::Style
    │
    ├── Inline props: class="box" → width: 200px
    ├── StyleBook match: tag < class < id priority
    └── Defaults: display=block, padding=0, margin=0
    │
    ▼
TaffyTree::insert_leaf() / insert_with_children()
    │
    ▼
taffy.compute_layout(root, available_space)
    │
    ▼
Vec<PositionedNode> — x, y, width, height per node
```

## Rendering Pipeline (Vello 0.10)

```
PositionedNode[]
    │
    ▼
SceneBuilder::build_scene():
  for node in positioned_nodes:
    match node.kind:
      Rect     → scene.fill(Rect, solid_brush(color))
      Text     → scene.fill(Parley text_layout, position)
      RoundR.  → scene.fill(RoundedRect, brush)
      Gradient → scene.fill(rect, linear_gradient_brush)
    │
    ▼
Renderer::render_frame():
  renderer.render_to_surface(
    &device, &surface, &scene,
    &RenderParams { width, height, base_color, aa_mode }
  )
    │
    ▼
Surface texture → Screen (60 FPS)
```

## CLI Pipeline

```
uwebr init my-app
  └── Scaffold: Cargo.toml, src/main.rs, src/app/App.uwebr

uwebr build [--release]
  └── .uwebr → transpiler::transpile() → .rs files
      ├── src/generated/app.rs (Element tree)
      ├── src/generated/mod.rs
      └── cargo build [--release]

uwebr check
  └── Parse all .uwebr files (validate only, no transpile)

uwebr dev
  └── File watcher (notify 7)
      ├── Initial: BuildCache::build_all()
      ├── On change: BuildCache::build_incremental(changed)
      └── 100ms debounce
```

## Performance Hedefleri

| Metrik | Hedef | Notlar |
|--------|-------|--------|
| İlk render | < 100ms | Cold start |
| Frame rate | 60 FPS | Sustained rendering |
| Memory | < 50MB | Typical desktop app |
| Binary size | < 10MB | Optimized release |
| Hot reload | < 500ms | File change → screen update |
| Layout compute | < 1ms | 1000 node tree |

## Güvenlik

- **Memory safety**: Rust'ın guarantee'ları
- **No null pointer**: Option<T> kullanımı
- **No data races**: Send + Sync bounds
- **Sandboxed rendering**: GPU pipeline isolate
- **Input validation**: XSS-style attacks yok (tarayıcı DOM'u yok)

---

*Son güncelleme: Ağustos 2026*
