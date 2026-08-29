# FAZ 17 — Runtime Hot-Swap + State Transfer

**Hedef:** Çalışan uygulamada shared library'yi unload/load ederek component'i yeniden yüklemek — process restart olmadan.

**Ön koşul:** FAZ 16 tamamlandı (shared library compile çalışıyor).

**Sebep:** FAZ 16 shared library üretiyor ama çalıştırmıyor. Bu faz, running app'in eski library'yi bırakıp yeni library'yi yüklemesini ve component fonksiyonunu swap etmesini sağlıyor.

**Tahmini süre:** ~3-4 saat
**Test hedefi:** +10-12 test

---

## Adım 1 — `uwebr-dynlib/src/loader.rs`

Shared library'yi runtime'da yükleyen modül:

```rust
use libloading::{Library, Symbol};
use std::path::Path;
use crate::abi::{RenderFn, CssFn, CleanupFn};

pub struct LoadedLibrary {
    _lib: Library,  // Drop edildiğinde unload olur
    render: RenderFn,
    css: Option<CssFn>,
    cleanup: Option<CleanupFn>,
}

impl LoadedLibrary {
    pub fn load(path: &Path) -> Result<Self> {
        unsafe {
            let lib = Library::new(path)?;
            let render: Symbol<RenderFn> = lib.get(b"render")?;
            let css: Option<Symbol<CssFn>> = lib.get(b"css").ok();
            let cleanup: Option<Symbol<CleanupFn>> = lib.get(b"cleanup").ok();
            Ok(Self {
                _lib: lib,
                render: *render,
                css: css.map(|s| *s),
                cleanup: cleanup.map(|s| *s),
            })
        }
    }

    pub fn render(&self) -> *mut u8 {
        unsafe { (self.render)() }
    }

    pub fn css(&self) -> Option<String> {
        unsafe {
            self.css.map(|css_fn| {
                let ptr = css_fn();
                if ptr.is_null() {
                    return None;
                }
                let mut len = 0;
                while *ptr.add(len) != 0 { len += 1; }
                let slice = std::slice::from_raw_parts(ptr, len);
                Some(String::from_utf8_lossy(slice).to_string())
            }).flatten()
        }
    }

    pub fn cleanup(&self) {
        unsafe {
            if let Some(cleanup_fn) = self.cleanup {
                cleanup_fn();
            }
        }
    }
}

impl Drop for LoadedLibrary {
    fn drop(&mut self) {
        self.cleanup();
        // Library drop edildiğinde libloading otomatik unload eder
    }
}
```

**Önemli notlar:**
- `Library` drop edildiğinde `libloading` otomatik unload eder
- Ama önce `cleanup()` çağrılmalı
- `render()` döndürdüğünü `Box::from_raw()` ile geri alıp free etmeliyiz

---

## Adım 2 — `uwebr-dynlib/src/swap.rs`

Hot-swap mantığı:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use crate::loader::LoadedLibrary;

pub struct HotSwapManager {
    current: Option<Arc<LoadedLibrary>>,
    library_dir: PathBuf,
    component_name: String,
}

impl HotSwapManager {
    pub fn new(library_dir: PathBuf, component_name: String) -> Self;

    /// İlk yükleme — mevcut library'yi load et
    pub fn load_initial(&mut self) -> Result<()>;

    /// Yeni library yükle ve eskiyi bırak
    /// 1. Yeni library'yi load et
    /// 2. Yeni CSS'i parse et (eğer değiştiyse)
    /// 3. Yeni render fonksiyonunu al
    /// 4. Eski library'yi drop et
    /// 5. Yeni library'yi current yap
    pub fn swap(&mut self, new_library_path: &Path) -> Result<SwapResult>;

    /// Mevcut component'i render et
    pub fn render(&self) -> *mut u8;

    /// Mevcut CSS'i al
    pub fn css(&self) -> Option<String>;

    /// Mevcut library yolunu al
    pub fn current_path(&self) -> Option<PathBuf>;
}

pub struct SwapResult {
    pub render_time_ms: u64,      // Swap sonrası ilk render
    pub css_changed: bool,        // CSS değişti mi
    pub old_library: Option<PathBuf>,
    pub new_library: PathBuf,
}
```

**Swap mantığı detayı:**

```
Eski Library (v1)  ──┐
                     ├──→ HotSwapManager.current = Library(v2)
Yeni Library (v2)  ──┘
```

1. Yeni library'yi load et (hâlâ eski library de açık)
2. Yeni `css()` fonksiyonunu çağır → yeni CSS string
3. Yeni CSS'i `StyleBook::parse` ile parse et
4. Eski `LoadedLibrary`'yi drop et → unload
5. `current`'i yeni library'ye ata

**Thread safety:** `LoadedLibrary` `Send + Sync` olmalı. `libloading::Library` zaten `Send`. Render fonksiyonu `unsafe extern "C"` olduğu için thread-safe.

---

## Adım 3 — `uwebr-dynlib/src/lib.rs`

Modülleri birleştir:

```rust
pub mod abi;
pub mod compiler;
pub mod loader;
pub mod swap;

pub use compiler::{compile_shared_library, CompileOptions, CompileProfile, CompileResult};
pub use loader::LoadedLibrary;
pub use swap::{HotSwapManager, SwapResult};
```

---

## Adım 4 — State transfer stratejisi

Component state'i (sayılar, stringler, booleanlar) Rust closure içinde yaşıyor. Hot-swap'ta state kaybolur — bu kabul edilebilir mi?

**Seçenek A — State yok (basit, bu FAZ'da uygulanacak):**
- Hot-swap her seferinde component'i sıfırdan render eder
- Kullanıcı state'i kaybolur (sayacı sıfırlar)
- avantaj: basit, güvenli, hızlı
- dezavantaj: UX kötü — her save'de state sıfırlanıyor

**Seçenek B — JSON state transfer (ileri FAZ):**
- Swap öncesi state'i JSON'a serialize et
- Swap sonrası JSON'dan deserialize et
- `uwebr-core`'a `serialize_state() -> serde_json::Value` ekle
- Component state'i restore et

Bu FAZ'da **Seçenek A** uygulanacak. Seçenek B gelecek FAZ'da.

---

## Adım 5 — Error recovery

Hot-swap sırasında hata oluşursa:

```rust
pub enum SwapError {
    CompileFailed { error: String },
    LoadFailed { path: PathBuf, error: String },
    SymbolNotFound { symbol: String },
    CssParseFailed { error: String },
}

impl HotSwapManager {
    /// Swap dene, hata olursa mevcut library'yi koru
    pub fn try_swap(&mut self, new_library_path: &Path) -> Result<SwapResult, SwapError> {
        // 1. Yeni library'yi dene
        let new_lib = LoadedLibrary::load(new_library_path)
            .map_err(|e| SwapError::LoadFailed { ... })?;

        // 2. CSS parse dene
        let css = new_lib.css();
        if let Some(ref css_str) = css {
            StyleBook::parse(css_str)
                .map_err(|e| SwapError::CssParseFailed { ... })?;
        }

        // 3. İlk render dene
        let ptr = new_lib.render();
        if ptr.is_null() {
            return Err(SwapError::SymbolNotFound { symbol: "render".into() });
        }
        // Pointer'ı serbest bırak
        unsafe { Box::from_raw(ptr); }

        // 4. Başarılı — eskiyi bırak, yenisini kabul et
        self.current = Some(Arc::new(new_lib));
        Ok(SwapResult { ... })
    }
}
```

**Kural:** Hata durumunda eski library korunur, kullanıcı eski haliyle çalışmaya devam eder. Konsola hata mesajı yazılır.

---

## Adım 6 — Library dosya adlandırma

Her hot-swap'ta yeni bir dosya oluşturmak dosya kilidi sorununa yol açar. Çözüm:

```
target/dynlib/
├── uwebr-dynlib_App_v1.dll
├── uwebr-dynlib_App_v2.dll
├── uwebr-dynlib_App_v3.dll
└── current.dll -> uwebr-dynlib_App_v3.dll  (symlink)
```

- Her versiyon benzersiz dosya adı alır (monotonik artan counter)
- `current.dll` symlink'i her zaman en son versiyonu gösterir
- Loader her zaman `current.dll`'den yükler
- Eski versiyonlar diskte kalır ama açık değildir

```rust
pub fn next_version_path(library_dir: &Path, component_name: &str, version: &mut u32) -> PathBuf {
    *version += 1;
    library_dir.join(format!("{}_{}_v{}.{}", crate::abi::LIB_PREFIX, component_name, version, crate::abi::library_extension()))
}
```

---

## Adım 7 — `uwebr-core`'a state serialization desteği (hafif)

State transfer için temel API (Seçenek B için hazırlık):

```rust
// uwebr-core/src/state.rs
pub fn snapshot_all() -> serde_json::Value {
    // Tüm state'leri JSON'a serialize et
}

pub fn restore_all(snapshot: &serde_json::Value) {
    // JSON'dan state'leri geri yükle
}
```

Bu FAZ'da sadece `snapshot_all` ve `restore_all` impl edilecek ama hot-swap'a bağlanmayacak. Gelecek FAZ'da bağlanacak.

---

## Adım 8 — Testler

### 8.1 LoadedLibrary::load testi
- FAZ 16'da üretilen test .dll'i load et
- `render` sembolu resolve edilmeli

### 8.2 LoadedLibrary::render testi
- `render()` null döndürmemeli
- Return edilen pointer `Box::from_raw` ile serbest bırakılabilmeli

### 8.3 LoadedLibrary::css testi
- CSS olan library'de `css()` doğru string'i döndürmeli
- CSS olmayan library'de `css()` None döndürmeli

### 8.4 HotSwapManager::load_initial testi
- İlk load başarılı olmalı
- `render()` çalışmalı

### 8.5 HotSwapManager::swap testi
- İlk load → swap → yeni render başarılı olmalı
- Eski library unload edilmeli (drop count ile doğrula)

### 8.6 swap error recovery testi
- Geçersiz library path → `SwapError::LoadFailed`
- Mevcut library korunmalı (hâlâ render edebilmeli)

### 8.7 css_changed flag testi
- CSS değiştiyse `css_changed: true`
- CSS değişmediyse `css_changed: false`

### 8.8 thread safety testi
- `LoadedLibrary` `Send` olmalı (compile-time check)

### 8.9 version counter testi
- `next_version_path` her çağrıldığında versiyon artmalı

### 8.10 cleanup çağrısı testi
- Library drop edildiğinde `cleanup()` fonksiyonu çağrılmalı

### 8.11 rapid swap testi
- 10 kez art arda swap — memory leak yok

### 8.12 null render testi
- `render()` null döndüren library → swap başarısız, eski korunmalı

---

## Doğrulama

1. `cargo test --workspace` → tüm testler yeşil
2. `cargo clippy --workspace` → 0 warning
3. `cargo fmt --check` → temiz
4. Manuel test zinciri:
   - `cargo run -p uwebr-cli -- compile --input scaffold/src/App.uwebr --output target/dynlib/`
   - Library dosyası oluşmalı
   - `LoadedLibrary::load()` ile load edilebilmeli
   - `render()` çağrısı başarılı olmalı
5. **libloading bağımlılığı:** `uwebr-dynlib/Cargo.toml`'a `libloading = "0.8"` ekle
