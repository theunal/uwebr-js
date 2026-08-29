# uwebr-js Durumu

> Son güncelleme: 28 Ağustos 2026 (FAZ 8)

## FAZ 6–9: JS→Rust dönüşüm boşlukları ✅ TAMAMLANDI (12/12)

| Sıra | FAZ | İşlem | Commit |
|------|-----|-------|--------|
| 1 | 6.1 | for-of / for-in desteği | d11514d |
| 2 | 6.2 | Object shorthand + method + getter/setter | d11514d |
| 3 | 7.1 | ?? operator → `.unwrap_or()` | d11514d |
| 4 | 7.2 | console.log/error/warn format fix | d11514d |
| 5 | 7.3 | Optional chaining null check | d11514d |
| 6 | 7.4 | Function expression → closure | 227ef43 |
| 7 | 8.1 | Iterator methods (.iter() ekleme) | 6295d59 |
| 8 | 8.2 | String methods mapping (10+ method) | 6295d59 |
| 9 | 9.1 | Object spread → HashMap::from_iter | 825fe22 |
| 10 | 8.3 | fetch/Promise, JSON.parse/stringify | f643778 |
| 11 | 9.2 | Class field type inference | 6769979 |
| 12 | 9.3 | Try/catch/throw → Result pattern | b52f16d |

## FAZ 8: `<script>` state lowering ✅ TAMAMLANDI

`.uwebr` `<script>` bloğu state'ini top-level'da tanımlar:

```js
let count = 0;
function increment() { count++; }
```

Bunu olduğu gibi yayınlamak modül kapsamında `let` üretir — Rust bunu reddeder, ayrıca `count` `increment` içinde kapsamda değildir. Yeni `script.rs` modülü her top-level binding'i reaktif accessor çiftine indirger:

```rust
fn __state_count() -> i64 { return uwebr_core::state::get("count".to_string(), 0); }
fn __set_state_count(value: i64) { uwebr_core::state::set("count".to_string(), value); }
fn increment() { __set_state_count(__state_count() + 1); }
```

Okumalar sinyale abone olur, yazmalar repaint tetikler.

### Yeni public API

```rust
pub fn transpile_script(js_code: &str) -> Result<ScriptResult>;

pub struct ScriptResult {
    pub code: String,
    pub warnings: Vec<String>,
    pub states: Vec<ScriptState>,     // lowered top-level binding'ler
    pub functions: Vec<String>,       // on:click={fn} bağlaması için
}
```

`uwebr-cli::transpiler` bu iki listeyi kullanarak `{count}` interpolasyonunu `__state_count()`'a, `on:click={increment}`'i `register_action`'a bağlar.

**Not:** `transpile()` (eski API) davranışını korur — top-level `let`'i olduğu gibi yayınlar. Yalnız `.uwebr` script blokları için `transpile_script()` kullanılmalı.

## 📊 İstatistik

- **Test:** 30/30 ✅ (17 unit + 13 integration)
- **Modüller:** analyzer, codegen, context, parser, **script** (yeni), transformer, types, utils

## Bilinen Sınırlar

- **Shadowing:** `script.rs` rewriting'i identifier tabanlıdır; bir fonksiyon içindeki `let v = ...` aynı adı taşıyan top-level binding ile karışabilir. Script blokları küçük ve shadowing seyrek olduğu için bilinçli bir tercih (`test_shadowed_local_still_rewrites_outer_reads` bunu sabitliyor).
- **Tip çıkarımı:** literal olmayan başlangıç değerleri `Type::Any` kalır ve `serde_json::Value` olarak yayınlanır — üretilen proje bu bağımlılığa sahip değildir. Sayı/string/bool literal'leri için çıkarım çalışır.
- **Async script:** `<script>` içindeki `async`/`await` transpile edilir ancak çalışma zamanı bir executor sağlamaz.

## 🔗 İlgili

Genel plan: `../../PLAN.md`
Mimari rehber: `../../ARCHITECTURE.md`
Son faz raporu: `../../faz8.plan.md`
