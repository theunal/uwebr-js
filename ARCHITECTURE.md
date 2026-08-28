# uwebr — Mimari Rehber

## Genel Bakış

uwebr, JavaScript/HTML/CSS kodunu alıp GPU ile çizilen masaüstü uygulamalarına çeviren bir Rust frameworküdür. Next.js benzeri bir geliştirici deneyimi sunar ancak tarayıcı yerine wgpu + vello ile doğrudan GPU'ya çizer.

## Temel Prensipler

1. **Tarayıcı Yok**: HTML/CSS/JS → Rust transformasyonu, tarayıcı DOM'u yok
2. **GPU-First**: Tüm çizimler wgpu + vello ile GPU'da yapılır
3. **Component-Based**: React/Leptos benzeri component model
4. **Reactive State**: Fine-grained signals (Leptos benzeri)
5. **File-Based Routing**: Next.js benzeri dosya tabanlı yönlendirme
6. **Cross-Platform**: Windows, macOS, Linux (aynı kod tabanı)

## Katmanlı Mimari

```
┌─────────────────────────────────────────────────────────┐
│                    Uygulama Katmanı                       │
│  pages/, components/, styles/ — Kullanıcı Kodu          │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                  Framework Katmanı                        │
│  uwebr-core: Signals, Components, Lifecycle, Routing    │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                 Transform Katmanı                         │
│  uwebr-html: HTML → rsx! AST                            │
│  uwebr-css:  CSS → Taffy Style                          │
│  uwebr-js:   JS → Rust AST (mevcut)                     │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                 Rendering Katmanı                         │
│  uwebr-render: Vello Scene + Taffy Layout + Parley Text │
└──────────────────────────────┬──────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────┐
│                 Platform Katmanı                          │
│  uwebr-app: Winit EventLoop + Wgpu Device + Window      │
└─────────────────────────────────────────────────────────┘
```

## Veri Akış Diyagramı

```
1. Girdi: HTML + CSS + JS dosyaları
   │
2. Parse: swc_html + lightningcss + swc_ecma_parser
   │  → HTML AST, CSS AST, JS AST
   │
3. Transform:
   │  uwebr-html:  HTML AST → HtmlNode tree
   │  uwebr-css:   CSS AST → Vec<(Selector, Style)>
   │  uwebr-js:    JS AST → RsStmt/RsExpr (mevcut)
   │
4. Component Resolution:
   │  HtmlNode tree + CSS selectors → Component tree
   │  Event handlers bağlanır
   │
5. Render Prep:
   │  Component tree → Virtual DOM
   │  Signal dependencies takip edilir
   │
6. Runtime Cycle (her frame):
   │  a. Signal changes → affected components re-render
   │  b. Virtual DOM diff → minimal patch list
   │  c. Patch list → updated Render tree
   │  d. taffy.compute_layout() → pixel positions
   │  e. vello::Scene building → draw commands
   │  f. wgpu render → GPU'ya gönder
   │
7. Çıktı: Ekranda piksel (60 FPS hedefi)
```

## Component Lifecycle

```
create_signal() ──→ Signal object (read/write)
       │
       ├── signal()      → Okunduğunda değeri döndür
       ├── set_signal()  → Yeni değer ayarla
       └── signal.update(|v| *v += 1) → Functional update
       │
create_memo() ──→ Derived signal (sadece read)
       │
       ├── Depends on input signals
       ├── Automatically recomputes when deps change
       └── Memoized (same input → same output, no recompute)
       │
create_effect() ──→ Side effect (runs when deps change)
       │
       ├── Runs once on mount
       ├── Re-runs when any read signal changes
       └── Cleanup function on re-run/destroy
       │
Component Mount:
       │
       ├── on_mount(|| { ... })     → Component mounted
       ├── on_cleanup(|| { ... })   → Component destroyed
       └── spawn(async { ... })     → Async task
```

## Virtual DOM Diff Algoritması

```
1. Eski component tree (önceki render)
2. Yeni component tree (mevcut render)
3. Her node için:
   ├── Aynı tip mi?
   │   ├── Evet → Props/children diff
   │   │   ├── Props aynı mı?
   │   │   │   ├── Evet → children diff (recursive)
   │   │   │   └── Hayır → Update props + children diff
   │   │   └── Children aynı mı?
   │   │       ├── Evet → No-op
   │   │       └── Hayır → Reconcile children
   │   └── Hayır → Unmount old + Mount new
   │
4. Diff sonucu: Patch list [Insert, Update, Remove]
5. Patch list'i Render tree'ye uygula
```

## Layout Hesaplama (Taffy)

```
Component Tree
      │
      ▼
Render Nodes (with style info)
      │
      ▼
Taffy Node Tree:
  ├── Root node (window size)
  │   └── Flex container
  │       ├── Child 1 (text)
  │       ├── Child 2 (button)
  │       └── Child 3 (list)
      │
      ▼
taffy.compute_layout(root, available_space)
      │
      ▼
Layout Output: Vec<(NodeId, Rect)>
  ├── Node 1: Rect { x: 0, y: 0, w: 800, h: 600 }
  ├── Node 2: Rect { x: 16, y: 16, w: 200, h: 40 }
  ├── Node 3: Rect { x: 16, y: 64, w: 120, h: 40 }
  └── ...
```

## Rendering Pipeline (Vello)

```
Layout Output (pixel positions)
      │
      ▼
Vello Scene Building:
  for each render node:
    match node:
      Rect → scene.fill(Rect::new(x, y, w, h), color)
      Text → scene.fill_text(text_layout, position)
      Image → scene.draw_image(image, rect)
      Clip → scene.push_clip(rect); children; scene.pop_clip()
      Shadow → scene.fill_shadow(shadow)
      │
      ▼
Vello Render:
  scene.encode() → GPU compute pipeline
      │
      ▼
wgpu render pass → Surface texture → Screen
```

## State Management Örneği

```rust
// Global state
let (theme, set_theme) = create_signal("dark".to_string());
provide_context(theme);

// Local state
#[component]
fn Counter() -> Element {
    let (count, set_count) = create_signal(0);
    let doubled = create_memo(move || *count() * 2);
    let theme = use_context::<Signal<String>>();

    create_effect(move |_| {
        println!("Count: {}, Theme: {}", count(), theme());
    });

    rsx! {
        div(class: theme().as_str()) {
            span { "Count: {count}" }
            span { "Doubled: {doubled}" }
            button(on:click = move |_| set_count.update(|c| *c += 1)) {
                "Increment"
            }
        }
    }
}
```

## Event Handling

```rust
// Mouse events
button(on:click = move |_| { /* handler */ })
div(on:mouseover = move |_| { /* handler */ })
div(on:mouseout = move |_| { /* handler */ })

// Keyboard events
input(on:keydown = move |e| {
    if e.key == Key::Enter {
        // submit
    }
})

// Input events
input(on:input = move |e| {
    let value = e.value.clone();
    set_name.set(value);
})

// Custom events
#[component]
fn CustomButton(on_action: Callback<i32>) -> Element {
    rsx! {
        button(on:click = move |_| on_action.emit(42)) {
            "Click me"
        }
    }
}
```

## Routing Sistemi

```
Dosya Yapısı:              Route:
src/pages/index.rs         → /
src/pages/about.rs         → /about
src/pages/blog/index.rs    → /blog
src/pages/blog/[slug].rs   → /blog/:slug (dynamic)
src/pages/dashboard/
  ├── index.rs             → /dashboard
  └── settings.rs          → /dashboard/settings
src/pages/404.rs           → fallback

Otomatik生成:
fn routes() -> RouteTree {
    route("/", index::Index)
    route("/about", about::About)
    route("/blog", blog::Index)
    route("/blog/:slug", blog::Slug)
    route("/dashboard", dashboard::Index)
    route("/dashboard/settings", dashboard::Settings)
    fallback(not_found::NotFound)
}
```

## Hot Reload Mekanizması

```
cargo uwebr dev
      │
      ▼
File Watcher (notify crate)
      │
      ├── .rs dosyası değişti → Incremental compile
      │   └── src/pages/*.rs → Sadece o sayfayı yeniden compile et
      │
      ├── .css dosyası değişti → CSS re-transform
      │   └── src/styles/*.css → Taffy style güncelle
      │
      └── .html dosyası değişti → HTML re-transform
          └── src/templates/*.html → rsx! AST güncelle
      │
      ▼
Runtime Hot Swap:
  1. Yeni component tree oluştur
  2. Virtual DOM diff (sadece değişenler)
  3. Minimal patch uygula
  4. Yeniden render (sadece değişen bölgeler)
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
| Diff compute | < 0.5ms | 1000 node diff |

## Güvenlik

- **Memory safety**: Rust'ın guarantee'ları
- **No null pointer**: Option<T> kullanımı
- **No data races**: Send + Sync bounds
- **Sandboxed rendering**: GPU pipeline isolate
- **Input validation**: XSS-style attacks yok (tarayıcı DOM'u yok)
- **Secret management**: Environment variables, keyring
