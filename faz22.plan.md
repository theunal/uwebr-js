# FAZ 22: Shared-Lib Transpiler Parity

## Problem

Hot-swap shared library yolundaki `transpile_uwebr()` fonksiyonu (`compiler.rs:407-455`) gerçek transpiler'ı (`uwebr-cli::transpiler::transpile`) KULLANMIYOR. Yerine basitleştirilmiş bir copy-paste versiyonu var:

- `<script>` state → `AtomicI32` stub (reaktif sinyal değil)
- Event handler → `/* handler */` boş gövde
- `{#each}`, `{#if}` directive'leri → desteklenmiyor
- `<Component />` composition → desteklenmiyor
- Sadece düz HTML çalışır

**Sonuç:** Hot-swap modda interaktif uygulama çalışmıyor.

## Çözüm

Gerçek transpiler pipeline'ını shared-lib yoluna bağla. `build` ve `dev --mode hot-swap` aynı kodu üretsin.

---

## Aşama 1: `transpile_uwebr()` → Gerçek Pipeline'a Geçiş

### 1.1 compiler.rs'deki `transpile_uwebr()` fonksiyonunu kaldır

**Dosya:** `crates/uwebr-dynlib/src/compiler.rs` (satır ~407-455)

Mevcut basitleştirilmiş transpilation:
```rust
fn transpile_uwebr(content: &str, component_name: &str, project_root: &Path) -> Result<String> {
    let _css = extract_css(content);
    let script = extract_tag(content, "script");
    let html = extract_html(content);
    // ... basit codegen
}
```

**Yerine:** `uwebr_cli::transpiler::transpile()` çağrısı. Bu zaten:
- HTML parse → directive expansion → JS analysis → Rust codegen yapıyor
- State accessor'ları, event handler'ları, component composition'u doğru üretiyor

### 1.2 `uwebr-dynlib` → `uwebr-cli` dependency ekle

**Dosya:** `crates/uwebr-dynlib/Cargo.toml`

```toml
[dependencies]
uwebr-cli = { workspace = true }
```

### 1.3 `transpile_uwebr()`'ü yeniden yaz

```rust
fn transpile_uwebr(content: &str, component_name: &str, _project_root: &Path) -> Result<String> {
    uwebr_cli::transpiler::transpile(content, component_name)
        .map_err(|e| anyhow::anyhow!("transpile failed: {e}"))
}
```

**Not:** `uwebr_cli::transpiler::transpile` fonksiyonunun public olduğundan emin ol. Eğer değilse, `pub` yap veya `lib.rs`'den export et.

---

## Aşama 2: `generate_lib_rs()` — Component Composition Desteği

### 2.1 `collect_component_refs` fonksiyonu ekle

**Dosya:** `crates/uwebr-dynlib/src/compiler.rs`

Mevcut `generate_lib_rs()` sadece tek bir root component üretiyor. Component composition için:
- HTML'deki `<Component />` tag'larını tara
- Her biri için import statement'ı üret
- `render()` fonksiyonunda composition'ı doğru bağla

```rust
fn collect_component_refs(html: &str) -> Vec<String> {
    // <Foo /> → "Foo" component ref'ini bul
    // Capitalized tag = component
}
```

### 2.2 `generate_lib_rs()` güncelle

Mevcut:
```rust
fn generate_lib_rs(css: &str, html: &str, handlers: &[String], script: &str, component_name: &str) -> String {
    // Sadece tek component, basit render
}
```

Yeni:
```rust
fn generate_lib_rs(
    css: &str,
    html: &str,
    handlers: &[String],
    script: &str,
    component_name: &str,
    component_refs: &[String],  // YENİ
) -> String {
    // Component import'ları
    // Composition render: main component + child components
    // State accessor'ları (script'ten gelen let → __state_xxx())
    // Event handler registration (handler → register_action)
}
```

---

## Aşama 3: Event Handler Wire-Up

### 3.1 Event handler stub'larını kaldır

**Dosya:** `crates/uwebr-dynlib/src/compiler.rs`

Mevcut (satır ~446-448):
```rust
for handler in &handlers {
    output.push_str(&format!("    register_action(\"{handler}\", {handler});\n"));
}
```

Bu zaten doğru — `register_action` çağrılıyor. Ama handler fonksiyonu boş:
```rust
fn increment() { /* handler */ }
```

**Çözüm:** `transpile()` zaten doğru handler gövdesi üretiyor. Transpiler parity ile bu sorun kalkar.

### 3.2 Handler registration sırasını kontrol et

Handler'lar `register_action` ile kaydedilmeli, component render'dan ÖNCE. Mevcut sıralama:
1. Script bindings (let → state)
2. Event handler registration
3. Component function

Bu sıralama doğru, sadece handler gövdelerinin boş olmaması gerekiyor.

---

## Aşama 4: State Persistence Across Swaps

### 4.1 Hot-swap sırasında state kaybı

Problem: Shared library swap edildiğinde `AtomicI32` static'ler sıfırlanıyor.

**Çözüm:** Swap öncesi state'i serialize et, swap sonrası deserialize et.

### 4.2 `state_snapshot` ve `state_restore` fonksiyonları

**Dosya:** `crates/uwebr-dynlib/src/swap.rs`

```rust
pub struct StateSnapshot {
    values: HashMap<String, Vec<u8>>,  // key → serialized value
}

impl HotSwapManager {
    pub fn save_state(&self, lib: &LoadedLibrary) -> StateSnapshot {
        // lib'deki tüm AtomicI32 static'leri oku
        // HashMap'e serialize et
    }

    pub fn restore_state(&self, lib: &LoadedLibrary, snapshot: &StateSnapshot) {
        // Yeni lib'deki static'leri snapshot değerlerine set et
    }
}
```

### 4.3 Shared lib'ye `export_state` / `import_state` sembolleri

**Dosya:** `crates/uwebr-dynlib/src/compiler.rs`

`generate_lib_rs`'e ekle:
```rust
// State serialization
#[no_mangle]
pub extern "C" fn export_state() -> *mut u8 {
    // HashMap → JSON byte array
}

#[no_mangle]
pub extern "C" fn import_state(data: *const u8, len: usize) {
    // JSON byte array → HashMap, AtomicI32'lere yaz
}
```

### 4.4 Loader'a state fonksiyonları ekle

**Dosya:** `crates/uwebr-dynlib/src/loader.rs`

```rust
pub type ExportStateFn = extern "C" fn() -> *mut u8;
pub type ImportStateFn = extern "C" fn(*const u8, usize);

pub struct LoadedLibrary {
    // ... mevcut
    export_state: Option<ExportStateFn>,
    import_state: Option<ImportStateFn>,
}
```

---

## Aşama 5: Double Compilation Düzeltme

### 5.1 Mevcut sorun

`dev_server_hot_swap`'ta:
- File watcher thread: compile → load → swap into Arc<Mutex<>>
- Main thread (`do_hot_swap`): compile BAŞTAN → load → swap

Yani her hot-swap'ta **2 kez** compile ediliyor.

### 5.2 Çözüm: Watcher sadece sinyal gönderiyor

**Dosya:** `crates/uwebr-cli/src/commands.rs`

`run_file_watcher`'ı basitleştir:
- Sadece dosya değişikliğini algıla
- `reload_tx.send(())` ile main thread'i bilgilendir
- Compile işlemini main thread yapsın (zaten `do_hot_swap` yapıyor)

Veya tam tersi: main thread sadece load+swap yapsın, compile watcher'da kalsın. Hangisi daha mantıklı?

**Karar:** Compile watcher'da kalsın (zaten yapıyor), main thread'deki `do_hot_swap`'taki gereksiz compile kaldırılsın. `do_hot_swap` sadece:
1. `lib_ref.lock()` ile mevcut library'yi al
2. CSS güncelle
3. Pipeline'ı yeniden oluştur
4. Redraw tetikle

---

## Aşama 6: Error Overlay in Window

### 6.1 Compile hatasıʾnda pencerede göster

**Dosya:** `crates/uwebr-cli/src/commands.rs`

Mevcut: Compile hatası stderr'e yazdırılıyor, pencere eski içeriği gösteriyor.

Yeni: Compile hatasıʾnda `HotSwapState`'e hata mesajı kaydet, render fonksiyonunda hata overlay'ı göster.

```rust
struct HotSwapState {
    // ... mevcut
    error_message: Option<String>,  // YENİ
}
```

Render fonksiyonunda:
```rust
fn render_frame(&mut self) {
    if let Some(err) = &self.error_message {
        // Kırmızı overlay + hata mesajı render et
        return;
    }
    // Normal render
}
```

### 6.2 Hata overlay tasarımı

- Tam ekran yarı-saydam siyah overlay
- Ortada kırmızı border'lı kutu
- İçinde hata mesajı (font: monospace, beyaz)
- İlk satır: "Compile Error"
- Kalan: rustc/cargo stderr çıktısı

---

## Aşama 7: Multi-Component Hot-Swap

### 7.1 Tüm .uwebr dosyalarını izle

**Dosya:** `crates/uwebr-cli/src/commands.rs`

Mevcut `run_file_watcher`: sadece `src/` dizinini izliyor.

Yeni: Proje dizinindeki TÜM `.uwebr` dosyalarını izle:
```rust
watcher.watch(root, RecursiveMode::Recursive)?;
// Filtre: .uwebr uzantılı dosyalar
```

### 7.2 Component registry

Her `.uwebr` dosyası ayrı bir shared lib üretir. Hot-swap sırasında:
1. Değişen dosyayı bul
2. İlgili shared lib'i compile et
3. Sadece o component'i swap et

---

## Aşama 8: Window Configuration

### 8.1 `uwebr.config.toml` desteği

**Dosya:** `crates/uwebr-cli/src/commands.rs`

```toml
# uwebr.config.toml
[window]
title = "My App"
width = 1024
height = 768
resizable = true
```

### 8.2 Config okuma

```rust
fn load_window_config(root: &Path) -> WindowConfig {
    let config_path = root.join("uwebr.config.toml");
    if config_path.exists() {
        // toml::from_str ile parse et
    } else {
        WindowConfig::default()  // 800x600, "uwebr"
    }
}
```

---

## Aşama 9: CLI Polish

### 9.1 Hata mesajları iyileştir

| Mevcut | Yeni |
|---|---|
| `"cargo build failed"` | `"cargo build failed for '{name}':\n{stderr}"` |
| `"compiled library not found"` | `"compiled library not found at {path}\nHint: Run `cargo build` first"` |
| `"deps dir not found"` | `"Dependencies not compiled. Run `cargo build` in the project first"` |
| `"No .uwebr files found"` | `"No .uwebr files found in {path}\nHint: Create one with `uwebr init {name}`"` |
| `"render failed"` | `"render failed: {actual_error_message}"` |

### 9.2 `dev` komutu için `--open` flag'i

```bash
uwebr dev --mode hot-swap --open examples/hello.uwebr
```

### 9.3 `build` komutu için `--output` flag'i

```bash
uwebr build --release --output dist/
```

---

## Dosya Değişiklikleri Özeti

| Dosya | Değişiklik |
|---|---|
| `crates/uwebr-dynlib/Cargo.toml` | `uwebr-cli` dependency ekle |
| `crates/uwebr-dynlib/src/compiler.rs` | `transpile_uwebr()` → gerçek pipeline, `generate_lib_rs()` composition, `export_state/import_state` |
| `crates/uwebr-dynlib/src/loader.rs` | `export_state`/`import_state` sembolleri |
| `crates/uwebr-dynlib/src/swap.rs` | `StateSnapshot`, save/restore |
| `crates/uwebr-cli/src/commands.rs` | Double compilation fix, error overlay, multi-component, config, CLI polish |
| `crates/uwebr-cli/src/lib.rs` | `pub use transpiler::transpile` (eğer yoksa) |
| `examples/hello.uwebr` | Interaktif example (buton + counter) |

---

## Sıralama

1. **Aşama 1** (Kritik): Transpiler parity — en temel eksiklik
2. **Aşama 5** (Kritik): Double compilation fix — performans
3. **Aşama 2** (Yüksek): Component composition
4. **Aşama 3** (Yüksek): Event handler wire-up (transpiler parity ile otomatik)
5. **Aşama 6** (Orta): Error overlay — developer experience
6. **Aşama 4** (Orta): State persistence — advanced
7. **Aşama 9** (Düşük): CLI polish
8. **Aşama 7** (Düşük): Multi-component
9. **Aşama 8** (Düşük): Window config

---

## Doğrulama

1. `cargo build --workspace` — derleme başarılı
2. `cargo test --workspace` — 1695+ test geçmeli
3. `cargo clippy --workspace` — temiz
4. `cargo fmt --check` — temiz
5. `cargo run --package uwebr-cli -- dev --mode hot-swap examples/hello.uwebr` — pencere açılmalı
6. Hot-swap test: examples/hello.uwebr'u düzenle → otomatik yenilenmeli
7. Interaktif test: butona tıkla → counter artmalı (state persistence ile)
8. Hata test: kasıtlı hatalı CSS yaz → pencerede error overlay görmeli
