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
├── PLAN.md                         # Bu dosya
├── ARCHITECTURE.md
└── crates/
    ├── uwebr-js/                   # ✅ JS/TS → Rust transpiler (13 test)
    ├── uwebr-html/                 # ✅ Iskelet: HTML parser + AST + rsx! codegen (5 test)
    ├── uwebr-css/                  # ✅ CSS parser → Taffy Style (32 test)
    ├── uwebr-core/                 # ✅ Iskelet: Signal, Component, Router, Context (5 test)
    ├── uwebr-render/               # ✅ Iskelet: wgpu + vello renderer (3 test)
    ├── uwebr-app/                  # ✅ Iskelet: App runner + window (2 test)
    └── uwebr-cli/                  # ✅ Iskelet: CLI binary (uwebr init/build/dev)
```

---

## 🛠️ Teknoloji Stack'i

| Katman | Crate | Versiyon | Neden |
|--------|-------|----------|-------|
| Pencere Yönetimi | `winit` | 0.30.x | Tüm Rust GUI frameworklerinin ortak altyapısı |
| GPU Soyutlama | `wgpu` | 30.x | WebGPU standardı, Vulkan/Metal/DX12/WebGPU |
| 2D Vektörel Çizim | `vello` | 0.10.0 | GPU compute-centric, 177 FPS |
| CPU Fallback | `vello_cpu` | 0.0.6 | GPU yokken CPU ile çizim |
| Text Yerleşimi | `parley` | 0.9.0 | Linebender ekosistemi |
| Layout Motoru | `taffy` | 0.14.0 | CSS Flexbox/Grid/Block |
| Biçimler | `kurbo` | 0.13.1 | Bezier eğrileri, vello entegrasyonu |
| HTML Parse | `markup5ever` | 0.14.1 | html5ever wrapper |
| CSS Parse | `lightningcss` | 1.0.0-alpha.72 | Firefox CSS parser'ı |
| JS Parse | `swc_ecma_parser` | 45.1 | ES2020+ parsing |
| Error Handling | `anyhow` + `thiserror` | - | - |
| CLI | `clap` | 4.x | - |

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
│  Parse Katmanı   │  markup5ever + lightningcss + swc_ecma_parser
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

## 🚀 Fazlara Ayrılmış Yol Haritası

### FAZ 0 — Workspace Kurulumu ✅ TAMAMLANDI
**Süre:** 1-2 saat
**Hedef:** Workspace yapısını oluştur

- [x] Root `Cargo.toml` oluştur (workspace, 7 member)
- [x] `uwebr-js`'i `crates/uwebr-js` altına taşı (13 test geçti)
- [x] Ortak dependency versiyonlarını paylaş
- [x] uwebr-html iskeleti: AST + parser + codegen (5 test)
- [x] uwebr-css iskeleti: AST + parser + codegen (4 test)
- [x] uwebr-core iskeleti: Signal, Component, Router, Context (5 test)
- [x] uwebr-render iskeleti: Renderer, Scene, Layout (3 test)
- [x] uwebr-app iskeleti: App, Window, Event (2 test)
- [x] uwebr-cli iskeleti: CLI binary (init/build/dev)
- [x] Workspace build + test (33/33 geçti)

### FAZ 1 — uwebr-html (Gerçek Parser) ✅ TAMAMLANDI
**Süre:** 1-2 hafta
**Hedef:** markup5ever ile gerçek HTML parsing

- [x] Iskelet: AST tanımları, basit hand-written parser, rsx! codegen
- [x] markup5ever + html5ever entegrasyonu (gerçek HTML5 parsing)
- [x] Namespace prefix handling (on:click, xmlns)
- [x] `{expression}` interpolasyon desteği
- [x] `{#each items as item}...{/each}` loop
- [x] `{#if condition}...{:else}...{/if}` conditional
- [x] `<Component />` component composition (PascalCase)
- [x] `{@html raw_html}` raw HTML insertion
- [x] `on:click={handler}` event handler attribute
- [x] Fragment desteği (`<>...</>`)
- [x] Integration tests (20 test)

### FAZ 2 — uwebr-css (CSS → Taffy) ✅ TAMAMLANDI
**Süre:** 1-2 hafta
**Hedef:** CSS → Taffy Style

- [x] Iskelet: basit CSS parser, property mapping
- [x] CSS parser: selector, property, value parsing (class, id, tag, universal, child, list, descendant)
- [x] CSS value parsing: px, em, rem, %, vw, vh, auto, hex/named colors, rgb(), hsl()
- [x] Shorthand support: padding/margin 1-4 values
- [x] Comment and @media support
- [x] CSS property → Taffy Style mapping:
  - [x] display (flex, grid, none)
  - [x] flex-direction, flex-wrap, flex-grow, flex-shrink
  - [x] justify-content, align-items, align-self
  - [x] gap, row-gap, column-gap
  - [x] padding/margin (shorthand + individual sides)
  - [x] width/height (Dimension), min/max-size (LengthPercentageAuto)
  - [x] position (relative, absolute), inset (top/right/bottom/left)
  - [x] overflow (visible, hidden, scroll, clip)
  - [x] border-radius, border-width
- [x] Taffy 0.14 API: LengthPercentage::length(), percent(), Rect<LengthPercentageAuto>, Size<Dimension>
- [x] Runtime API: `convert_to_taffy_styles(rules) -> Vec<(String, Style)>`
- [x] Codegen API: `generate_taffy_styles(rules) -> String`
- [x] 32 tests (20 parser + 12 codegen)
- [x] Tamamlandı: FAZ 2 ✅

### FAZ 3 — uwebr-core (Reactive System)
**Süre:** 2-3 hafta
**Hedef:** State management + lifecycle

- [x] Iskelet: Signal, Component, Router, Context
- [ ] `#[component]` macro (proc-macro)
- [ ] `#[derive(Props)]` macro
- [ ] create_effect (reactive side effects)
- [ ] Virtual DOM diffing
- [ ] Event system (on:click, on:input)
- [ ] Spawn/async desteği
- [ ] Integration tests

### FAZ 4 — uwebr-render (GPU Pipeline)
**Süre:** 3-4 hafta
**Hedef:** GPU rendering pipeline

- [x] Iskelet: Renderer, Scene, LayoutEngine
- [ ] Vello scene builder
- [ ] Rect/RoundRect/Line çizimi
- [ ] Text rendering (parley + vello)
- [ ] Gradient, Shadow, Opacity, Transform
- [ ] Image rendering
- [ ] Hit testing
- [ ] Scroll container
- [ ] Taffy layout integration
- [ ] Benchmarks

### FAZ 5 — uwebr-app (Window + Events)
**Süre:** 2 hafta
**Hedef:** Pencere + event loop

- [x] Iskelet: App, Window, Event
- [ ] Winit ApplicationHandler
- [ ] GPU device initialization
- [ ] RedrawRequested → render pipeline
- [ ] Mouse/Keyboard event dispatch
- [ ] Multi-window desteği
- [ ] Timer/animation frame

### FAZ 6 — uwebr-cli (Developer Experience)
**Süre:** 1 hafta
**Hedef:** Developer experience

- [x] Iskelet: init/build/dev komutları
- [ ] `create` komutu (scaffolding, template)
- [ ] `dev` komutu (hot reload, notify)
- [ ] `build` komutu (production binary)
- [ ] Dosya değişikliği algılama
- [ ] Incremental rebuild

---

## 📊 Durum Tablosu

| Component | Durum | Test | Not |
|-----------|-------|------|-----|
| uwebr-js | ✅ Tamamlandı | 13/13 | JS→Rust transpiler, tüm FAZ'lar tamam |
| uwebr-html | ✅ Tamamlandı | 20/20 | FAZ 1: markup5ever, template directives, components |
| uwebr-css | ✅ Tamamlandı | 32/32 | FAZ 2: CSS parser + Taffy Style mapping |
| uwebr-core | 🔄 Geliyor | 5/5 iskelet | FAZ 3: proc-macro bekliyor |
| uwebr-render | 🔄 Geliyor | 3/3 iskelet | FAZ 4: vello entegrasyonu bekliyor |
| uwebr-app | 🔄 Geliyor | 2/2 iskelet | FAZ 5: winit ApplicationHandler bekliyor |
| uwebr-cli | 🔄 Geliyor | - | FAZ 6: scaffolding + hot reload bekliyor |

**Toplam:** 75/75 test geçti

---

## 🔗 Referanslar

- [Taffy](https://github.com/DioxusLabs/taffy) — CSS Flexbox/Grid layout engine
- [Vello](https://github.com/linebender/vello) — GPU compute 2D renderer
- [Parley](https://github.com/linebender/parley) — Text layout
- [Wgpu](https://github.com/gfx-rs/wgpu) — WebGPU abstraction
- [Winit](https://github.com/rust-windowing/winit) — Window management
- [LightningCSS](https://github.com/parcel-bundler/lightningcss) — CSS parser
- [markup5ever](https://github.com/servo/rust-html5ever) — HTML5 parser
- [Leptos](https://github.com/leptos-rs/leptos) — Reference for signals/component model
- [Slint](https://slint.dev/) — Reference for declarative UI
- [Xilem](https://github.com/linebender/xilem) — Reference for reactive UI with vello

---

*Son güncelleme: Ağustos 2026*
