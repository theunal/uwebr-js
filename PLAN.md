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
├── PLAN.md                         # Bu dosya — yol haritası ve durum
├── ARCHITECTURE.md                 # Mimari rehber: katmanlar, pipeline, doğrulama
├── faz4.plan.md                    # FAZ 4 planı + plandan sapmalar
├── faz8.plan.md                    # FAZ 8 raporu + bulgu doğrulama tablosu
└── crates/
    ├── uwebr-js/                   # ✅ JS/TS → Rust transpiler + script state lowering (30 test)
    ├── uwebr-html/                 # ✅ HTML parser + template directives + components (31 test)
    ├── uwebr-css/                  # ✅ CSS parser → Taffy Style + PaintProps (59 test)
    ├── uwebr-core/                 # ✅ Reactive system + Timer + script state + actions (80 test)
    ├── uwebr-macro/                # ✅ #[component] + #[derive(Props)] (5 test)
    ├── uwebr-render/               # ✅ Layout + Scene + Text + StyleBook + Paint (93 test)
    ├── uwebr-app/                  # ✅ Multi-window + GpuContext + RenderPipeline (79 test)
    └── uwebr-cli/                  # ✅ CLI: init/build/check/dev + transpiler (72 test)
```

---

## 🛠️ Teknoloji Stack'i

| Katman | Crate | Versiyon | Neden |
|--------|-------|----------|-------|
| Pencere Yönetimi | `winit` | 0.30.x | Tüm Rust GUI frameworklerinin ortak altyapısı |
| GPU Soyutlama | `wgpu` | 29.x | vello 0.10'un beklediği sürüm; farklı bir major çakışan tipler üretir |
| 2D Vektörel Çizim | `vello` | 0.10.0 | GPU compute-centric |
| Text Yerleşimi | `parley` | 0.9.0 | Linebender ekosistemi |
| Layout Motoru | `taffy` | 0.14.0 | CSS Flexbox/Grid/Block |
| Biçimler | `kurbo` | 0.11.x | Bezier eğrileri (vello re-export eder) |
| HTML Parse | `html5ever` | 0.29 | Gerçek HTML5 parser |
| CSS Parse | Custom | - | Hand-written (lightningcss alpha API kararsız) |
| JS Parse | `swc_ecma_parser` | 45.1 | ES2020+ parsing |
| Error Handling | `anyhow` + `thiserror` | - | - |
| CLI | `clap` | 4.x | - |
| File watching | `notify` | 7.0 | `uwebr dev` |

**wgpu sürüm notu:** `uwebr-render` FAZ 8'e kadar wgpu 30'a bağlıydı; vello 0.10 wgpu 29 kullandığı için derlemede iki ayrı wgpu kopyası vardı. `uwebr-render` artık GPU'ya hiç dokunmadığından bağımlılık kaldırıldı; GPU yalnız `uwebr-app`'te (wgpu 29).

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
│  uwebr-css       │  CSS rules → (Taffy Style, StyleMask, PaintProps)
│  StyleBook       │  tag < class < id, yalnız set edilmiş property'ler
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  uwebr-render    │  Element → TaffyTree → PositionedNode
│  Layout          │  compute_layout_with_measure → parley ölçümü
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  uwebr-render    │  PositionedNode → vello::Scene
│  Scene Builder   │  fill(), stroke(), draw_glyphs()
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  uwebr-app       │  vello → Rgba8Unorm storage texture
│  GpuContext      │  → TextureBlitter → surface texture
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

### FAZ 2 — uwebr-css ✅ TAMAMLANDI (59 test)
- [x] Custom CSS parser: selector, property, value
- [x] Selectors: tag, class, id, universal, child, descendant, list
- [x] Values: px, em, rem, %, vw, vh, auto, hex/named colors, rgb(), hsl()
- [x] Shorthand: padding/margin 1-4 values
- [x] Properties: display, flex-*, justify-*, align-*, gap, width/height, position, overflow, border-*
- [x] `convert_to_taffy_styles(rules) -> Vec<(String, Style)>`
- [x] **FAZ 8:** `convert_to_style_entries()` → `StyleEntry { selector, style, mask, paint }`
- [x] **FAZ 8:** `PaintProps` — background/color/font-size/font-family/border/opacity
- [x] **FAZ 8:** `rem` artık `em` kolunda yakalanmıyor (`"2rem"` → `"2r"` parse hatası düzeltildi)

### FAZ 3 — uwebr-core ✅ TAMAMLANDI (80 test)
- [x] Signal: create_signal, get, set, update, clone
- [x] Memo: create_memo (lazy, dependency tracking)
- [x] Effect: create_effect (reactive side effects)
- [x] Context: provide_context / use_context (TypeId-based)
- [x] Router: add_route, navigate, resolve
- [x] Virtual DOM diffing
- [x] Event system: on:click, on:input
- [x] Lifecycle: on_mount, on_cleanup, with_component
- [x] `use_signal` / `use_memo` hooks
- [x] **FAZ 8:** `state` modülü — isimle anahtarlı script state (`get` / `set` / `clear`)
- [x] **FAZ 8:** Named actions — `register_action` / `dispatch_action` / `has_action`
- [x] **FAZ 8:** Render dirty bayrağı — `mark_render_dirty` / `take_render_dirty`

### FAZ 3.5 — Timer/Animation Frame ✅ TAMAMLANDI
- [x] `TimerRegistry` — global timer collection (Arc<Mutex<>>)
- [x] `set_timeout` — one-shot timer
- [x] `set_interval` — repeating timer
- [x] `request_animation_frame` — vsync-aligned callback
- [x] `cancel_timer` — cancel by handle
- [x] App integration: tick in new_events(), fire in RedrawRequested

### FAZ 4 — uwebr-render ✅ TAMAMLANDI (93 test)
- [x] `color.rs` — CSS Color → peniko::Color
- [x] `scene.rs` — RenderScene, RenderNode, RenderStyle
- [x] `text.rs` — Parley layout + measure (+ fontsuz ortam için tahmin)
- [x] `paint.rs` — ResolvedPaint: renk/font kalıtımı, CSS < inline önceliği
- [x] `layout.rs` — LayoutEngine, `compute_layout_with_measure` ile metin ölçümü
- [x] `scene_builder.rs` — PositionedNode → vello Scene, `draw_glyphs` ile metin
- [x] `renderer.rs` — Scene assembler (GPU state yok; o `uwebr-app`'te)

### FAZ 4.5 — CSS Integration (StyleBook) ✅ TAMAMLANDI
- [x] `StyleBook` — parse CSS, match elements by tag/class/id
- [x] Priority: tag < class < id
- [x] `StyleMask` — yalnız kuralda belirtilmiş property'ler yazılır (cascade düzeltmesi)
- [x] `PaintProps` — background/color/font-size/font-family Taffy sınırında düşmez
- [x] `LayoutEngine::build_tree()` accepts `&StyleBook`
- [x] `RenderPipeline::with_css()` / `with_stylebook()` API

### FAZ 5 — uwebr-app ✅ TAMAMLANDI (79 test)
- [x] Winit ApplicationHandler (resumed, window_event, about_to_wait)
- [x] GPU device initialization (wgpu 29 + vello 0.10)
- [x] `GpuContext`: Rgba8Unorm storage texture + `TextureBlitter` ile surface'e blit
- [x] RenderPipeline: Element → Layout → Scene → vello Scene
- [x] Component trait + FnComponent
- [x] Mouse/Keyboard event dispatch
- [x] `on:click` hit-testing → `dispatch_action`
- [x] Signal-dirty repaint (`take_render_dirty` + timer)
- [x] Multi-window: `HashMap<WindowId, WindowState>`
- [x] `open_window()` API — queue windows for creation on resume

### FAZ 6 — uwebr-cli ✅ TAMAMLANDI (72 test)
- [x] `uwebr init <name>` — scaffolding + ilk transpile (derlenebilir çıktı)
- [x] `uwebr build [--release]` — transpile .uwebr → .rs + cargo build
- [x] `uwebr check` — validate-only (parse all .uwebr files)
- [x] `uwebr dev` — file watching (notify 7) + transpile + build + **uygulamayı yeniden başlat**
- [x] `BuildCache` — incremental parse cache (dev_server tanılaması), 100ms debounce
- [x] Build hatasında çalışan uygulama ayakta kalır
- [x] **Transpiler:** .uwebr → Rust Element tree codegen

### FAZ 7 — Transpiler (Production Build) ✅ TAMAMLANDI
- [x] `transpiler::transpile(content, name) -> Result<String>`
- [x] HTML → `Element { node_type, props, children }` tree
- [x] CSS → `pub const CSS_NAME: &str` embedding
- [x] Script → gerçek Rust (top-level `let` → reaktif accessor'lar)
- [x] `{count}` interpolasyonu → `__state_count()` sinyal okuması
- [x] `on:click={fn}` → `PropValue::Closure` + `register_action`
- [x] Auto-generate: main.rs, mod.rs, component function
- [x] Handles: elements, text, attributes, fragments, components (slot children), each/if
- [x] Comment / `{@html}` node'ları geçersiz Rust üretmiyor
- [x] Attribute/metin değerleri Rust string literal'i için escape ediliyor

### FAZ 8 — Son Kilometre: Ekranda Görünen Uygulama ✅ TAMAMLANDI
- [x] **M1** Metin render: Taffy measure function + parley `draw_glyphs`
- [x] **M2** CSS boyası ekrana çıkıyor: `PaintProps` → `ResolvedPaint` → scene
- [x] **M3** Cascade düzeltmesi: `StyleMask` ile yalnız belirtilmiş property'ler
- [x] **M4** GPU yolu: storage texture + blit (doğrulandı, önceki hali runtime'da panikliyordu)
- [x] **M5** Gerçek hot reload: süreç spawn/kill/respawn
- [x] **M6** `<script>` ↔ template ↔ reaktivite: state lowering, `{count}`, `on:click`, dirty repaint
- [x] **M7** Scaffold derlenebilir: `src/generated/` + path dependency + ilk transpile

Ayrıntılı rapor, bulgu doğrulama tablosu ve açık maddeler: [`faz8.plan.md`](faz8.plan.md)

### Tanılama örnekleri

FAZ 8'de eklendi; "gerçekten ekrana çıkıyor mu?" sorusunu otomatikleştirir:

```bash
cargo run -p uwebr-render --example glyph_probe    # glyph üretimi + ölçüm
cargo run -p uwebr-render --example layout_probe   # font-size → text box yüksekliği
cargo run -p uwebr-app --example gpu_probe         # headless GPU render + framebuffer analizi
cargo run -p uwebr-cli --example scaffold_output   # scaffold'ın ürettiği Rust
```

---

## 📊 Durum Tablosu

| Crate | Durum | Test | Özellikler |
|-------|-------|------|------------|
| uwebr-js | ✅ Tamamlandı | 30/30 | JS→Rust transpiler, script state lowering |
| uwebr-html | ✅ Tamamlandı | 31/31 | HTML parser, template directives |
| uwebr-css | ✅ Tamamlandı | 59/59 | CSS parser → Taffy Style + PaintProps |
| uwebr-core | ✅ Tamamlandı | 80/80 | Signals, effects, lifecycle, timer, state, actions |
| uwebr-macro | ✅ Tamamlandı | 5/5 | #[component], #[derive(Props)] — testleri `uwebr-core/tests/` |
| uwebr-render | ✅ Tamamlandı | 93/93 | Layout, scene, text, paint, stylebook |
| uwebr-app | ✅ Tamamlandı | 79/79 | Multi-window, GpuContext, pipeline, hit-test |
| uwebr-cli | ✅ Tamamlandı | 72/72 | Transpiler, scaffold, hot reload |

**Toplam:** 444/444 test geçti (`cargo test --workspace`, 28 Ağustos 2026)

Kalite kapıları: `cargo fmt --all --check` temiz; `cargo clippy --workspace --all-targets` FAZ 8 dosyalarında uyarı üretmiyor (kalan 21 uyarı faz öncesi dosyalarda).

### Çalıştırma ile doğrulananlar

| Ne | Nasıl | Sonuç |
|----|-------|-------|
| Metin + CSS ekrana çıkıyor | `cargo run -p uwebr-app --example gpu_probe` | 17 glyph, `#1a1a2e` arka plan, 947 px `#e0e0e0` |
| Scaffold derleniyor | `uwebr init demo && cargo build` | başarılı |
| Uygulama açılıyor | üretilen binary | 7 s çalıştı, panik yok |
| Hot reload | `uwebr dev` + dosya değişimi | yeni PID, 6.9 s |
| Tıklama → state → yeniden render | `cargo test -p uwebr-app --test interaction_tests` | 8 test |

Ölçülmeyenler: FPS, bellek kullanımı, binary boyutu, cold start süresi, 1000 node layout süresi.


---

## 📋 CLI Referansı

```bash
# Yeni proje oluştur (scaffold + ilk transpile)
uwebr init my-app
cd my-app

# Geliştirme modu: dosya izle → transpile → build → uygulamayı yeniden başlat
uwebr dev

# Production build
uwebr build --release

# Validate-only
uwebr check
```

### Bilinen sınırlar

- **Hot reload süreç yeniden başlatmalıdır.** Değişiklik başına ~7 s (`cargo build` baskın). `< 500 ms` hedefi ancak in-process reload ile mümkün; şu an tutulmuyor.
- **Component props callee'ye geçirilmiyor.** `<Card title="x" />` prop'u `Element.props`'a yazılıyor ama `card_component()` argüman almıyor. Slot children FAZ 8'de düzeltildi.
- **`{@html expr}`** node'ları sahneye çıkmıyor (gerçek bir HTML alt-parser'ı gerekiyor); geçersiz Rust üretmiyor, sessizce düşüyor.
- **Pseudo-class / attribute selector'lar** parse ediliyor ama eşleşmede yok sayılıyor (`.btn:hover` → `.btn` gibi davranır).
- **`vw`/`vh`** yüzde olarak yaklaşılıyor: kökte viewport'a, iç içe elementlerde ebeveyne göre çözülür.
- **`overflow: hidden`** scene tarafında kırpıyor, ancak `RenderStyle::overflow_hidden` CSS'ten doldurulmuyor.
- **Gradient** CSS'ten gelmiyor. `Background::LinearGradient` desteği var, `uwebr-css` `linear-gradient(...)`'ı `Keyword` olarak saklıyor.
- **`RenderNodeKind::Image`** gerçek görsel çizmiyor, `Rect` olarak düşüyor.
- **Script shadowing:** state rewriting identifier tabanlı; fonksiyon içindeki aynı adlı local top-level binding ile karışabilir.

### Olası sonraki adımlar

Bunlar planlanmadı, yalnız yukarıdaki sınırların doğal karşılıkları:

1. **Component props** — props struct'ı + `#[component]` makro entegrasyonu; `uwebr-macro` şu an kullanılmıyor.
2. **In-process hot reload** — `< 500 ms` hedefini gerçekten tutmak için tek yol.
3. **Performans ölçümü** — FPS/bellek/binary boyutu için bir benchmark harness'ı; hedefler şu an doğrulanmamış.
4. **CSS kapsamı** — gradient, `overflow` boyaya taşıma, pseudo-class eşleştirme, gerçek `vw`/`vh`.
5. **Görsel** — `Image` node'u için dekodlama + `scene.draw_image`.


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

## 📚 Belge Haritası

| Belge | İçerik |
|-------|--------|
| `PLAN.md` | Yol haritası, faz durumları, test sayıları, bilinen sınırlar |
| `ARCHITECTURE.md` | Katmanlar, veri akışı, reaktif sistem, GPU yolu, doğrulama durumu |
| `faz4.plan.md` | FAZ 4 (`uwebr-render`) planı ve plandan yedi sapma |
| `faz8.plan.md` | FAZ 8 raporu: her bulgunun akıbeti, ölçümler, açık maddeler |
| `crates/uwebr-js/GAPS_PLAN.md` | uwebr-js JS→Rust boşlukları + script state lowering |

---

*Son güncelleme: 28 Ağustos 2026 (FAZ 8)*
