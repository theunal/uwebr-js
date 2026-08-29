# FAZ 16 — Dynamic Library Crate (Shared Library Output)

**Hedef:** .uwebr dosyalarını shared library (.dll/.so/.dylib) olarak derleyen `uwebr-dynlib` crate'i oluşturmak.

**Sebep:** Şu anki hot reload: dosya değişimi → `cargo build` (~6.5s) → process restart. Bu faz, `cargo build` adımını shared library compile'a düşürerek gelecek FAZ'larda in-process hot-swap'ı mümkün kılacak.

**Tahmini süre:** ~2-3 saat
**Test hedefi:** +6-8 test

---

## Adım 1 — Crate iskeleti

`crates/uwebr-dynlib/` oluştur:

```
crates/uwebr-dynlib/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── compiler.rs
    └── abi.rs
```

`Cargo.toml`:
```toml
[package]
name = "uwebr-dynlib"
description = "Shared library compiler for uwebr hot reload"
license.workspace = true
version.workspace = true
edition.workspace = true

[dependencies]
anyhow.workspace = true
tempfile = "3"
log = "0.4"

[dev-dependencies]
tempfile.workspace = true
```

Workspace'e ekle: `Cargo.toml` members listesine `"crates/uwebr-dynlib"`.

---

## Adım 2 — ABI tanımı (`abi.rs`)

Shared library'nin dışarıya açacağı fonksiyonları tanımla:

```rust
/// Component fonksiyonunun shared library'den beklediği imza.
/// String argument incorrectly used as pointer here; it should be raw pointer.
pub type RenderFn = unsafe extern "C" fn() -> *mut u8;

/// CSS string'ini döndüren fonksiyon (varsa).
pub type CssFn = unsafe extern "C" fn() -> *const u8;

/// Library unload edilmeden önce çağrılan cleanup.
pub type CleanupFn = unsafe extern "C" fn();
```

Bu fonksiyonlar shared library tarafından export edilecek, runtime tarafında `libloading` ile resolve edilecek.

---

## Adım 3 — Compiler (`compiler.rs`)

`.uwebr` dosyasını shared library'ye compile eden fonksiyon:

```rust
pub struct CompileOptions {
    pub root: PathBuf,           // Proje root'u
    pub target_dir: PathBuf,     // .dll/.so çıktısı dizini
    pub profile: CompileProfile, // Debug veya Release
}

pub enum CompileProfile {
    Debug,
    Release,
}

pub struct CompileResult {
    pub library_path: PathBuf,   // Üretilen .dll/.so yolu
    pub compile_time_ms: u64,    // Compile süresi
    pub css: Option<String>,     // CSS içeriği (hot-swap için)
}

pub fn compile_shared_library(
    uwebr_content: &str,
    component_name: &str,
    options: &CompileOptions,
) -> Result<CompileResult>;
```

Compile süreci:
1. `tempfile::TempDir` oluştur
2. Geçici bir Cargo projesi oluştur (`cargo init --lib`)
3. `Cargo.toml`'a `uwebr-core`, `uwebr-render` path dependency olarak ekle
4. `src/lib.rs`'e transpile edilmiş component kodunu yaz
5. `#[no_mangle] pub extern "C" fn render() -> *mut u8` export fonksiyonu ekle
6. `#[no_mangle] pub extern "C" fn css() -> *const u8` export fonksiyonu ekle (CSS varsa)
7. `cargo build --lib` çalıştır (`--release` veya debug)
8. Üretilen .dll/.so dosyasını `target_dir`'e kopyala
9. Sonucu döndür

**Önemli:** Transpile adımı mevcut `uwebr-cli::transpiler::transpile()` fonksiyonunu kullanacak — kod tekrarı yok.

---

## Adım 4 — Export fonksiyon kalıbı

Üretilen `src/lib.rs` şu kalıba uygun olmalı:

```rust
#![allow(unused)]

use uwebr_core::component::{Element, PropValue};
use uwebr_core::events::register_action;

// ... transpile edilmiş component kodu ...

// CSS varsa
#[no_mangle]
pub extern "C" fn css() -> *const u8 {
    // static CSS_XXX: &str = r#"..."#;
    CSS_APP.as_ptr()
}

#[no_mangle]
pub extern "C" fn render() -> *mut u8 {
    let elem = app_component(&[]);
    // Element'i Box ile sarmala ve pointer döndür
    Box::into_raw(Box::new(elem))
}

#[no_mangle]
pub extern "C" fn cleanup() {
    // Gerekli temizlik
}
```

**Not:** `Element` `Send + 'static` olmalı ki cross-thread transfer mümkün olsun. Bu zaten mevcut.

---

## Adım 5 — Platform-specific library uzantısı

```rust
pub fn library_extension() -> &'static str {
    #[cfg(target_os = "windows")]
    { "dll" }
    #[cfg(target_os = "macos")]
    { "dylib" }
    #[cfg(target_os = "linux")]
    { "so" }
}

pub fn library_filename(component_name: &str) -> String {
    format!("{}_{component_name}", env!("CARGO_PKG_NAME"))
    // uwebr-dynlib_App.dll
}
```

---

## Adım 6 — CLI entegrasyonu

`uwebr-cli`'ye yeni komut:

```bash
# Tek .uwebr dosyasını shared library olarak compile et
uwebr compile --input src/App.uwebr --output target/dynlib/

# Tüm .uwebr dosyalarını compile et
uwebr compile-all
```

`commands.rs`'e ekle:

```rust
pub fn compile_library(path: &str, output: &str) -> Result<()> {
    let content = fs::read_to_string(path)?;
    let name = Path::new(path).file_stem().unwrap().to_str().unwrap();
    let options = CompileOptions {
        root: PathBuf::from(path).parent().unwrap().to_path_buf(),
        target_dir: PathBuf::from(output),
        profile: CompileProfile::Debug,
    };
    let result = compile_shared_library(&content, name, &options)?;
    println!("Compiled in {}ms → {}", result.compile_time_ms, result.library_path.display());
    Ok(())
}
```

---

## Adım 7 — Testler

### 7.1 CompileResult testi
- `compile_shared_library` çağrılabilmeli (geçici dizinde)
- `library_path` dosyası mevcut olmalı
- `compile_time_ms` > 0 olmalı

### 7.2 Export fonksiyon testi
- Üretilen .dll load edilmeli (`libloading::Library`)
- `render` sembolu bulunabilmeli
- `css` sembolu bulunabilmeli (CSS varsa)

### 7.3 Render çağrısı testi
- `render()` fonksiyonu çağrılmalı
- Return edilen pointer `null` olmamalı
- `cleanup()` çağrılmalı

### 7.4 CSS export testi
- CSS olan bir .uwebr'den üretilen library'de `css` fonksiyonu döndürülen string ile eşleşmeli

### 7.5 Platform extension testi
- `library_extension()` doğru uzantıyı döndürmeli

### 7.6 Compile retry testi
- Aynı input ile iki kez compile edilmeli (dosya kilidi yok)

### 7.7 Hatalı input testi
- Geçersiz .uwebr content → `compile_shared_library` `Err` döndürmeli

---

## Doğrulama

1. `cargo test --workspace` → tüm testler yeşil
2. `cargo clippy --workspace` → 0 warning
3. `cargo fmt --check` → temiz
4. Manuel test: `cargo run -p uwebr-cli -- compile --input scaffold/src/App.uwebr --output target/dynlib/` → .dll üretmeli
5. Oluşan .dll'i `dumpbin /exports` (Windows) veya `nm -D` (Linux) ile kontrol et
