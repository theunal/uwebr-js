# uwebr — Rust Native Desktop App Framework

> Next.js benzeri DX, %100 Rust, GPU ile çizim. Tarayıcı yok, HTML yok.

---

## 🎯 Vizyon

JavaScript/TypeScript/HTML/CSS kodunu alıp, doğrudan GPU ile çizilen masaüstü uygulamalarına çeviren bir ekosistem. Next.js'in geliştirici deneyimini (dosya tabanlı yönlendirme, component model, hot reload) Rust'a taşıyıp, tarayıcı yerine wgpu + vello ile ekrana çizen bir framework.

---

## 📁 Workspace Yapısı

```
uwebr/
├── Cargo.toml                      # Workspace root (8 member)
├── PLAN.md                         # Bu dosya
└── crates/
    ├── uwebr-js/                   # ✅ JS/TS → Rust transpiler (13 test)
    ├── uwebr-html/                 # ✅ HTML parser + template directives + components (31 test)
    ├── uwebr-css/                  # ✅ CSS parser → Taffy Style (43 test)
    ├── uwebr-core/                 # ✅ Reactive system + Timer (54 test)
    ├── uwebr-macro/                # ✅ #[component] + #[derive(Props)] (5 test)
    ├── uwebr-render/               # ✅ GPU pipeline + StyleBook (47 test)
    ├── uwebr-app/                  # ✅ Multi-window + RenderPipeline (31 test)
    └── uwebr-cli/                  # ✅ CLI: init/build/check/dev + transpiler (28 test)
```

---

## 🛠️ Teknoloji Stack'i

| Katman | Crate | Versiyon | Neden |
|--------|-------|----------|-------|
| Pencere Yönetimi | `winit` | 0.30.x | Tüm Rust GUI frameworklerinin ortak altyapısı |
| GPU Soyutlama | `wgpu` | 29.x | WebGPU standardı, Vulkan/Metal/DX12/WebGPU |
| 2D Vektörel Çizim | `vello` | 0.10.0 | GPU compute-centric, 177 FPS |
| Text Yerleşimi | `parley` | 0.9.0 | Linebender ekosistemi |
| Layout Motoru | `taffy` | 0.14.0 | CSS Flexbox/Grid/Block |
| Biçimler | `kurbo` | 0.13.x | Bezier eğrileri, vello entegrasyonu |
| HTML Parse | `html5ever` | 0.29 | Gerçek HTML5 parser |
| CSS Parse | Custom | - | Hand-written (lightningcss alpha API kararsız) |
| JS Parse | `swc_ecma_parser` | 45.1 | ES2020+ parsing |
| Error Handling | `anyhow` + `thiserror` | - | - |
| CLI | `clap` | 4.x | - |

### Veri Akışı

```
.uwebr Dosyası (HTML + <script> + <style>)
        │
        ▼
┌──────────────────┐
│  uwebr-html      │  html5ever → Element AST
│  Parse           │  {#each}, {#if}, <Component/>
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  uwebr-css       │  CSS rules → Taffy Style
│  StyleBook       │  tag < class < id priority
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  uwebr-render    │  Element → TaffyTree → PositionedNode
│  Layout          │  (taffy 0.14 compute_layout)
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  uwebr-render    │  PositionedNode → vello::Scene
│  Scene Builder   │  fill(), stroke(), gradients
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  uwebr-app       │  wgpu + vello → GPU render
│  Renderer        │  Scene → surface texture
└──────────────────┘
         │
         ▼
    GPU Ekranı
```

---

## 🚀 Fazlara Ayrılmış Yol Haritası

### FAZ 0 — Workspace Kurulumu ✅ TAMAMLANDI
- [x] Root `Cargo.toml` (workspace, 8 member)
- [x] Tüm crate iskeletleri
- [x] Ortak dependency versiyonları

### FAZ 1 — uwebr-html ✅ TAMAMLANDI (31 test)
- [x] markup5ever + html5ever ile gerçek HTML5 parsing
- [x] `{expression}` interpolasyon
- [x] `{#each items as item}...{/each}` loop
- [x] `{#if condition}...{:else}...{/if}` conditional
- [x] `<Component />` composition (PascalCase detection)
- [x] `{@html raw_html}` raw insertion
- [x] `on:click={handler}` event handlers
- [x] Fragment desteği (`<>...</>`)
- [x] Block directive reassembly (html5ever text node splitting)

### FAZ 2 — uwebr-css ✅ TAMAMLANDI (43 test)
- [x] Custom CSS parser: selector, property, value
- [x] Selectors: tag, class, id, universal, child, descendant, list
- [x] Values: px, em, rem, %, vw, vh, auto, hex/named colors, rgb(), hsl()
- [x] Shorthand: padding/margin 1-4 values
- [x] Properties: display, flex-*, justify-*, align-*, gap, width/height, position, overflow, border-*
- [x] `convert_to_taffy_styles(rules) -> Vec<(String, Style)>`

### FAZ 3 — uwebr-core ✅ TAMAMLANDI (54 test)
- [x] Signal: create_signal, get, set, update, clone
- [x] Memo: create_memo (lazy, dependency tracking)
- [x] Effect: create_effect (reactive side effects)
- [x] Context: provide_context / use_context (TypeId-based)
- [x] Router: add_route, navigate, resolve
- [x] Virtual DOM diffing
- [x] Event system: on:click, on:input
- [x] Lifecycle: on_mount, on_cleanup, with_component
- [x] `use_signal` / `use_memo` hooks

### FAZ 3.5 — Timer/Animation Frame ✅ TAMAMLANDI
- [x] `TimerRegistry` — global timer collection (Arc<Mutex<>>)
- [x] `set_timeout` — one-shot timer
- [x] `set_interval` — repeating timer
- [x] `request_animation_frame` — vsync-aligned callback
- [x] `cancel_timer` — cancel by handle
- [x] App integration: tick in new_events(), fire in RedrawRequested

### FAZ 4 — uwebr-render ✅ TAMAMLANDI (47 test)
- [x] `color.rs` — CSS Color → peniko::Color (6 test)
- [x] `scene.rs` — RenderScene, RenderNode, RenderStyle (6 test)
- [x] `text.rs` — Parley + Vello text rendering (4 test)
- [x] `layout.rs` — LayoutEngine (TaffyTree wrapper) (7 test)
- [x] `scene_builder.rs` — PositionedNode → vello Scene (8 test)
- [x] `renderer.rs` — GPU pipeline (wgpu + vello) (6 test)

### FAZ 4.5 — CSS Integration (StyleBook) ✅ TAMAMLANDI
- [x] `StyleBook` — parse CSS, match elements by tag/class/id
- [x] Priority: tag < class < id
- [x] `LayoutEngine::build_tree()` accepts `&StyleBook`
- [x] `RenderPipeline::with_css()` / `with_stylebook()` API
- [x] 8 StyleBook tests

### FAZ 5 — uwebr-app ✅ TAMAMLANDI (31 test)
- [x] Winit ApplicationHandler (resumed, window_event, about_to_wait)
- [x] GPU device initialization (wgpu + vello)
- [x] RenderPipeline: Element → Layout → Scene → vello Scene
- [x] Component trait + FnComponent
- [x] Mouse/Keyboard event dispatch
- [x] Multi-window: `HashMap<WindowId, WindowState>`
- [x] `open_window()` API — queue windows for creation on resume
- [x] Per-window event dispatch + close

### FAZ 6 — uwebr-cli ✅ TAMAMLANDI (28 test)
- [x] `uwebr init <name>` — scaffolding (Cargo.toml, main.rs, App.uwebr)
- [x] `uwebr build [--release]` — transpile .uwebr → .rs + cargo build
- [x] `uwebr check` — validate-only (parse all .uwebr files)
- [x] `uwebr dev` — file watching (notify 7) + incremental rebuild
- [x] `BuildCache` — incremental parse cache, 100ms debounce
- [x] **Transpiler:** .uwebr → Rust Element tree codegen (10 tests)

### FAZ 7 — Transpiler (Production Build) ✅ TAMAMLANDI
- [x] `transpiler::transpile(content, name) -> Result<String>`
- [x] HTML → `Element { node_type, props, children }` tree
- [x] CSS → `const CSS_NAME: &str` embedding
- [x] Script → comment block (JS→Rust via uwebr-js TODO)
- [x] Auto-generate: main.rs, mod.rs, component function
- [x] Handles: elements, text, attributes, fragments, components, each/if blocks

---

## 📊 Durum Tablosu

| Crate | Durum | Test | Özellikler |
|-------|-------|------|------------|
| uwebr-js | ✅ Tamamlandı | 13/13 | JS→Rust transpiler |
| uwebr-html | ✅ Tamamlandı | 31/31 | HTML parser, template directives |
| uwebr-css | ✅ Tamamlandı | 43/43 | CSS parser → Taffy Style |
| uwebr-core | ✅ Tamamlandı | 54/54 | Signals, effects, lifecycle, timer |
| uwebr-macro | ✅ Tamamlandı | 5/5 | #[component], #[derive(Props)] |
| uwebr-render | ✅ Tamamlandı | 47/47 | Layout, scene, text, renderer |
| uwebr-app | ✅ Tamamlandı | 57/57 | Multi-window, GPU, Component |
| uwebr-cli | ✅ Tamamlandı | 33/33 | Transpiler, incremental rebuild |

**Toplam:** 283/283 test geçti (unit + integration)

---

## 📋 CLI Referansı

```bash
# Yeni proje oluştur
uwebr init my-app
cd my-app

# Geliştirme modu (hot reload)
uwebr dev

# Production build
uwebr build --release

# Validate-only
uwebr check
```

---

## 🔗 Referanslar

- [Taffy](https://github.com/DioxusLabs/taffy) — CSS Flexbox/Grid layout engine
- [Vello](https://github.com/linebender/vello) — GPU compute 2D renderer
- [Parley](https://github.com/linebender/parley) — Text layout
- [Wgpu](https://github.com/gfx-rs/wgpu) — WebGPU abstraction
- [Winit](https://github.com/rust-windowing/winit) — Window management
- [markup5ever](https://github.com/servo/rust-html5ever) — HTML5 parser
- [Leptos](https://github.com/leptos-rs/leptos) — Signals/component model reference
- [Slint](https://slint.dev/) — Declarative UI reference
- [Xilem](https://github.com/linebender/xilem) — Reactive UI with vello reference

---

*Son güncelleme: Ağustos 2026*
