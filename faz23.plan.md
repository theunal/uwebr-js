# FAZ 23: State Persistence (Hot-Reload State Korunması)

## Problem
`do_hot_swap()` her değişimde shared library'yi yeniden yükler, state (`count`, `name` vs.) sıfırlanır. Kullanıcı sayfayı yenilediğinde counter 0'a döner.

## Çözüm Mimarisi

### Katman 1: State Serileştirme (`uwebr-core/src/state.rs`)
`SCRIPT_STATE`'a erişim için iki yeni public fonksiyon:
- `export_state() -> String` — JSON formatında tüm state'i serialize eder
- `import_state(json: &str)` — JSON'dan state'i geri yükler

`Box<dyn Any>` değerleri `i64`, `f64`, `bool`, `String` tiplerinde deserialize edilebilir.

### Katman 2: Shared Library ABI (`uwebr-dynlib/src/compiler.rs`)
`generate_lib_rs()` iki yeni `extern "C"` fonksiyon ekler:
- `export_state() -> *const c_char` — JSON string döndürür
- `import_state(json: *const c_char)` — JSON'dan state'i yükler

### Katman 3: Loader & Swap Manager
`LoadedLibrary`'ye iki yeni alan + fonksiyon:
- `ExportStateFn` / `ImportStateFn` type alias'ları
- `export_state()` / `import_state()` method'ları

### Katman 4: Hot-Swap Flow (`commands.rs`)
`do_hot_swap()` sırası:
1. Eski library'den `export_state()` çağır → JSON al
2. Yeni library'yi yükle
3. Yeni library'ye `import_state(json)` çağır

### Katman 5: Transpiler (`uwebr-transpiler`)
State metadata'sı (`ScriptState`) zaten her binding'in tipini biliyor.
`generate_lib_rs()` state metadata'sını kullanarak tip-aware JSON yazacak.

## Adımlar

### 23.1 State Serileştirme (uwebr-core)
**Dosya:** `crates/uwebr-core/src/state.rs`
- `export_state()` fonksiyonu: `SCRIPT_STATE`'ı iterate edip JSON array üretir
  - Format: `[{"key":"count","type":"i64","value":42}, ...]`
  - `Box<dyn Any>` → `&dyn Any` downcast ile tip kontrolü
- `import_state(json)` fonksiyonu: JSON parse edip `SCRIPT_STATE`'a yazar
- `serde_json` dependency ekle (workspace'te zaten var)
- Testler: export/import roundtrip, eksik key handling

### 23.2 Shared Library ABI Genişletme
**Dosya:** `crates/uwebr-dynlib/src/abi.rs`
- `ExportStateFn = unsafe extern "C" fn() -> *const c_char`
- `ImportStateFn = unsafe extern "C" fn(*const c_char)`

**Dosya:** `crates/uwebr-dynlib/src/compiler.rs`
- `generate_lib_rs()`'e `export_state()` ve `import_state()` extern "C" fonksiyonları ekle

**Dosya:** `crates/uwebr-dynlib/src/loader.rs`
- `LoadedLibrary`'ye `export_state: Option<ExportStateFn>`, `import_state: Option<ImportStateFn>` ekle
- `export_state()` / `import_state()` method'ları

### 23.3 Hot-Swap State Transfer
**Dosya:** `crates/uwebr-cli/src/commands.rs`
- `do_hot_swap()`: swap öncesi eski library'den state al, swap sonrası yeni library'ye yaz
- `HotSwapManager::try_swap_with_state()` — yeni variant

### 23.4 Testler
- `state::tests::test_export_import_roundtrip`
- `state::tests::test_export_empty_state`
- `state::tests::test_import_overwrites`
- `loader::tests::test_export_state_fn_type`
- `swap::tests::test_try_swap_preserves_state`

## Değişen Dosyalar
| Dosya | Değişiklik |
|-------|-----------|
| `crates/uwebr-core/src/state.rs` | `export_state()`, `import_state()` |
| `crates/uwebr-core/Cargo.toml` | `serde_json` ekle |
| `crates/uwebr-dynlib/src/abi.rs` | Yeni type alias'lar |
| `crates/uwebr-dynlib/src/compiler.rs` | `generate_lib_rs()` genişletme |
| `crates/uwebr-dynlib/src/loader.rs` | `LoadedLibrary` genişletme |
| `crates/uwebr-dynlib/src/swap.rs` | `try_swap_with_state()` |
| `crates/uwebr-cli/src/commands.rs` | `do_hot_swap()` state transfer |
