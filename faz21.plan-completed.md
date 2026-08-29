# FAZ 21 — Hot-Swap Shared Library → Window Rendering

**Hedef:** `.uwebr` dosyasını compile edip sıcak değişim (hot-swap) ile gerçek pencerede GPU ile göstermek.

**Ön koşul:** FAZ 20 tamamlandı (direct rustc compile, ~340ms). `uwebr-app`'de mevcut `GpuContext`, `App`, `RenderPipeline` zaten var.

**Sebep:** Şu an `bench-reload` shared lib'i yüklüyor, `render()` çağırıyor, `Element` alıyor ama **pencere açmıyor**. `uwebr-app::App` pencere açabiliyor ama **shared lib yüklemiyor**. FAZ 21'de bunları birleştiriyoruz.

---

## Mimari

### Mevcut Durum
```
bench-reload:  compile → load lib → render() → Element → (yok et)
uwebr-app:     App → GpuContext → RenderPipeline → vello::Scene → GPU
```

### Yeni (FAZ 21)
```
dev --mode hot-swap:
  compile → load lib → App penceresi aç
  ┌──────────────────────────────────────┐
  │  winit Event Loop                    │
  │  ┌─────────┐  ┌──────────┐  ┌─────┐ │
  │  │ Loaded  │→ │ Render   │→ │ GPU │ │
  │  │ Library │  │ Pipeline │  │     │ │
  │  └─────────┘  └──────────┘  └─────┘ │
  │       ↑ file change → recompile     │
  └──────────────────────────────────────┘
```

---

## Adım 1 — `hot-swap` modu: uwebr-app ile entegrasyon

### 1.1 CLI: `dev --mode hot-swap` pencere açsın

Mevcut `dev_server_hot_swap` sadece konsolda log basar. Yeni:
- `uwebr_app::App` oluştur
- İlk compile sonucunu `LoadedLibrary` olarak yükle
- `FnComponent` ile sar — `render()` çağrıldığında shared lib'deki `render()` fonksiyonunu çağırsın
- winit event loop'a gir

### 1.2 HotSwapComponent

```rust
pub struct HotSwapComponent {
    library: Arc<Mutex<LoadedLibrary>>,
}

impl Component for HotSwapComponent {
    fn render(&self) -> Element {
        let lib = self.library.lock().unwrap();
        match lib.render() {
            Some(elem) => elem,
            None => Element::text("render failed"),
        }
    }
}
```

### 1.3 Dosya değişikliğinde yeniden compile + swap

File watcher tetiklendiğinde:
1. Yeni shared lib compile et (rustc, ~340ms)
2. Yeni lib'i yükle
3. Eski lib'i drop et (otomatik cleanup)
4. Pencereyi yeniden çiz (request_redraw)

---

## Adım 2 — `dev --mode restart` için de destek

Mevcut `dev_server_with_mode` zaten `restart` modunu destekliyor. FAZ 21'de:
- `hot-swap` modu: winit event loop + shared lib + file watcher
- `restart` modu: her değişiklikte tam restart (mevcut davranış)

---

## Adım 3 — CSS hot-reload (basit)

Dosya değişikliğinde CSS'i de yeniden parse et:
1. `.uwebr` dosyasından `<style>` bloğunu extract et
2. `StyleBook`'u güncelle
3. Pencereyi yeniden çiz

---

## Adım 4 — Minimal example: `examples/hello.uwebr`

Test için basit bir `.uwebr` dosyası:
```html
<div class="app">
  <h1>Hello from uwebr!</h1>
  <p>Hot-swap: edit this file and save.</p>
</div>

<style>
.app { background: #1a1a2e; color: #e0e0e0; padding: 2rem; }
h1 { color: #00d4ff; font-size: 2rem; }
p { color: #888; }
</style>
```

---

## Adım 5 — Testler

### 5.1 HotSwapComponent unit test
- `render()` çağrıldığında shared lib'den Element döndürmeli
- Library reload sonrası yeni Element döndürmeli

### 5.2 Entegrasyon testi (headless)
- GpuContext oluşturmadan pipeline testi
- Element → Scene dönüşümü

### 5.3 Manuel E2E test
- `cargo run -- dev --mode hot-swap examples/hello.uwebr`
- Pencere açılmazsa → FAIL
- Pencere açılıp render olmazsa → FAIL

---

## Dosyalar

| Dosya | Değişiklik |
|-------|-----------|
| `crates/uwebr-cli/src/commands.rs` | `dev_server_hot_swap` → winit loop + HotSwapComponent |
| `crates/uwebr-app/src/lib.rs` | `HotSwapComponent` ekle (veya `component.rs`) |
| `crates/uwebr-app/src/component.rs` | `HotSwapComponent` struct + Component impl |
| `examples/hello.uwebr` | Minimal test dosyası |

---

## Doğrulama

1. `cargo test --workspace` → tüm testler yeşil
2. `cargo clippy --workspace` → 0 warning
3. `cargo fmt --check` → temiz
4. `cargo run --package uwebr-cli -- dev --mode hot-swap examples/hello.uwebr` → pencere açılır, render görünür
5. Dosyayı değiştir → otomatik güncellenir
