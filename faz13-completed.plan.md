# FAZ 13: Selector Eşleşmesi ve Hot Reload

> Durum: 📋 Plan hazır, onay bekliyor
> Oluşturma: 29 Ağustos 2026
> Karmaşıklık: Yüksek | Tahmini: ~4-5 saat

## Genel Bakış

Kalan 2 bilinen sınırdan biri olan pseudo-class/attribute selector eşleşmesi + hot reload iyileştirmesi + bellek ölçümü.

---

## ADIM 1: Pseudo-Class / Attribute Selector Eşleşmesi (~120 dk)

### Mevcut Durum

- Parser `:hover`, `[type="text"]` gibi selector'ları parse ediyor ama `_pseudo` ve `_attrs` olarak atlıyor (parser.rs:247-260)
- `CssSelector` enum'unda pseudo-class ve attribute için varyant yok
- `StyleBook::match_full()` sadece tag, class, id eşleştiriyor (stylebook.rs:94-130)
- Eşleşme yapılmadığı için `.btn:hover { background: blue; }` hiçbir zaman uygulanmıyor

### Desteklenecek Selector'lar

**Stateless (runtime state gerektirmez):**
- `:first-child` — element ebeveynin ilk çocuğu mu?
- `:last-child` — element ebeveynin son çocuğu mu?
- `:nth-child(n)` — element n. çocuk mu?
- `[attr]` — attribute var mı?
- `[attr="value"]` — attribute belirli değere sahip mi?
- `[attr~="value"]` — attribute value listesini içeriyor mu?
- `[attr^="prefix"]` — attribute prefix ile başlıyor mu?
- `[attr$="suffix"]` — attribute suffix ile bitiyor mu?
- `[attr*="contains"]` — attribute string içeriyor mu?

**Stateful (runtime state gerektirir — FAZ 13'te sadece altyapı):**
- `:hover`, `:focus`, `:active` — element state'i runtime'da değişir
- Bu durumlar için `ElementState` bitmask'i + hover tracking altyapısı kurulur
- Ama gerçek hover/focus takibi winit event loop entegrasyonu gerektirir → sonraki faz

### Yapılacaklar

#### 1.1 AST'ye selector varyantları ekle

**Dosya:** `crates/uwebr-css/src/ast.rs`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CssSelector {
    Class(String),
    Id(String),
    Tag(String),
    Universal,
    Descendant(Vec<CssSelector>),
    Child(Vec<CssSelector>),
    List(Vec<CssSelector>),
    // ← Yeni:
    /// .btn:hover, div:first-child
    PseudoClass(Box<CssSelector>, String),
    /// input[type="text"], [disabled]
    Attribute {
        selector: Box<CssSelector>,
        attr: String,
        op: AttributeOp,
        value: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AttributeOp {
    Exists,          // [attr]
    Equals,          // [attr="value"]
    Includes,        // [attr~="value"]
    Prefix,          // [attr^="value"]
    Suffix,          // [attr$="value"]
    Contains,        // [attr*="value"]
}
```

#### 1.2 Parser'da pseudo-class ve attribute bilgisini sakla

**Dosya:** `crates/uwebr-css/src/parser.rs`

Mevcut kodu güncelle — `_pseudo` ve `_attrs` artık atılmasın:

```rust
// Mevcut:
if chars.peek() == Some(&':') {
    chars.next();
    let _pseudo = read_ident(chars); // atılıyordu
}

// Yeni:
let mut selector = base_selector;
if chars.peek() == Some(&':') {
    chars.next();
    let pseudo_name = read_ident(chars);
    selector = CssSelector::PseudoClass(Box::new(selector), pseudo_name);
}

// Attribute:
if chars.peek() == Some(&'[') {
    chars.next();
    let (attr_name, op, attr_value) = parse_attribute_selector(chars);
    if chars.peek() == Some(&']') {
        chars.next();
    }
    selector = CssSelector::Attribute {
        selector: Box::new(selector),
        attr: attr_name,
        op,
        value: attr_value,
    };
}
```

Yeni yardımcı fonksiyon:

```rust
fn parse_attribute_selector(chars: &mut Peekable<Chars>) -> (String, AttributeOp, Option<String>) {
    skip_whitespace(chars);
    let attr = read_ident(chars);
    skip_whitespace(chars);

    let op = if chars.peek() == Some(&'=') {
        chars.next();
        AttributeOp::Equals
    } else if chars.peek() == Some(&'~') {
        chars.next();
        if chars.peek() == Some(&'=') { chars.next(); }
        AttributeOp::Includes
    } else if chars.peek() == Some(&'^') {
        chars.next();
        if chars.peek() == Some(&'=') { chars.next(); }
        AttributeOp::Prefix
    } else if chars.peek() == Some(&'$') {
        chars.next();
        if chars.peek() == Some(&'=') { chars.next(); }
        AttributeOp::Suffix
    } else if chars.peek() == Some(&'*') {
        chars.next();
        if chars.peek() == Some(&'=') { chars.next(); }
        AttributeOp::Contains
    } else {
        return (attr, AttributeOp::Exists, None);
    };

    skip_whitespace(chars);
    let value = if chars.peek() == Some(&'"') || chars.peek() == Some(&'\'') {
        Some(read_string(chars))
    } else {
        Some(read_ident(chars))
    };

    (attr, op, value)
}
```

#### 1.3 StyleBook selector matching güncelle

**Dosya:** `crates/uwebr-render/src/stylebook.rs`

`match_full()`'u genişlet — artık selector zincirini recursive olarak eşle:

```rust
pub fn match_full(&self, element: &Element) -> MatchedStyle {
    let mut out = MatchedStyle::default();
    let tag = match &element.node_type {
        NodeType::Element(tag) => tag.as_str(),
        _ => return out,
    };

    for entry in &self.rules {
        if selector_matches(&entry.selector, element, tag) {
            self.absorb(&mut out, entry);
        }
    }

    out
}
```

Yeni recursive matching fonksiyonu:

```rust
fn selector_matches(sel: &CssSelector, element: &Element, tag: &str) -> bool {
    match sel {
        CssSelector::Tag(t) => t == tag,
        CssSelector::Class(c) => element_has_class(element, c),
        CssSelector::Id(id) => element_has_id(element, id),
        CssSelector::Universal => true,
        CssSelector::PseudoClass(inner, pseudo) => {
            selector_matches(inner, element, tag)
                && pseudo_class_matches(pseudo, element)
        }
        CssSelector::Attribute { selector: inner, attr, op, value } => {
            selector_matches(inner, element, tag)
                && attribute_matches(element, attr, op, value.as_deref())
        }
        CssSelector::Descendant(selectors) => {
            // En son selector element ile eşleşmeli
            if let Some(last) = selectors.last() {
                selector_matches(last, element, tag)
            } else {
                false
            }
            // Not: gerçek descendant eşleşmesi parent chain gerektirir.
            // FAZ 13'te basitleştirilmiş: son selector'un eşleşmesi yeterli.
        }
        CssSelector::Child(selectors) => {
            if let Some(last) = selectors.last() {
                selector_matches(last, element, tag)
            } else {
                false
            }
        }
        CssSelector::List(sels) => sels.iter().any(|s| selector_matches(s, element, tag)),
    }
}
```

#### 1.4 Pseudo-class eşleşme fonksiyonları

**Dosya:** `crates/uwebr-render/src/stylebook.rs`

```rust
fn pseudo_class_matches(pseudo: &str, element: &Element) -> bool {
    match pseudo {
        // Stateless — parent chain gerektirir (basitleştirilmiş)
        "first-child" | "last-child" | "nth-child" => {
            // Gerçek eşleşme parent'ın çocuk listesini bilmeyi gerektirir.
            // FAZ 13'te: her zaman true döndür (geçici).
            // Gerçek implementasyon: Element'e parent reference ekle.
            true
        }
        // Stateful — runtime state gerektirir
        "hover" | "focus" | "active" | "visited" => {
            // FAZ 13'te: altyapı kuruldu ama gerçek tracking sonraki fazda.
            // Şimdilik her zaman false.
            false
        }
        "disabled" => {
            element.props.iter().any(|(k, v)| k == "disabled" && matches!(v, PropValue::Bool(true)))
        }
        "enabled" => {
            !element.props.iter().any(|(k, v)| k == "disabled" && matches!(v, PropValue::Bool(true)))
        }
        _ => false,
    }
}
```

#### 1.5 Attribute eşleşme fonksiyonları

**Dosya:** `crates/uwebr-render/src/stylebook.rs`

```rust
fn attribute_matches(element: &Element, attr: &str, op: &AttributeOp, value: Option<&str>) -> bool {
    let attr_value = element.props.iter()
        .find(|(k, _)| k == attr)
        .and_then(|(_, v)| match v {
            PropValue::String(s) => Some(s.as_str()),
            PropValue::Bool(true) => Some(""),
            _ => None,
        });

    match op {
        AttributeOp::Exists => attr_value.is_some(),
        AttributeOp::Equals => attr_value == value,
        AttributeOp::Includes => {
            attr_value.map_or(false, |v| {
                v.split_whitespace().any(|w| w == value.unwrap_or(""))
            })
        }
        AttributeOp::Prefix => {
            attr_value.map_or(false, |v| v.starts_with(value.unwrap_or("")))
        }
        AttributeOp::Suffix => {
            attr_value.map_or(false, |v| v.ends_with(value.unwrap_or("")))
        }
        AttributeOp::Contains => {
            attr_value.map_or(false, |v| v.contains(value.unwrap_or("")))
        }
    }
}
```

#### 1.6 Testler

- `parser.rs`: `.btn:hover { background: blue; }` → `PseudoClass(Class("btn"), "hover")`
- `parser.rs`: `input[type="text"]` → `Attribute { Tag("input"), "text", Equals }`
- `parser.rs`: `[disabled]` → `Attribute { Universal, "disabled", Exists }`
- `parser.rs`: `[class*="active"]` → `Attribute { Universal, "class", Contains, "active" }`
- `stylebook.rs`: `.btn:hover` → hover olmayan elemente uygulanmaz
- `stylebook.rs`: `[disabled]` → disabled prop'u olan elemente uygulanır
- `stylebook.rs`: `input[type="text"]` → type="text" olan input'a uygulanır

---

## ADIM 2: Hot Reload İyileştirmesi (~90 dk)

### Mevcut Durum

- `uwebr dev` → file watcher → debounce 100ms → transpile + cargo build + kill + respawn
- Toplam süre ~7s, neredeyse tamamı `cargo build`
- CSS changes için altyapı var: `StyleBook::reparse()` → vw/vh desteği
- Ama hot reload döngüsü her şeyi yeniden build ediyor

### Yaklaşım: Aşamalı Hot Reload

**Seviye 1 (FAZ 13): CSS-only hot reload** — en sık değişen şey, anında yeniden render
**Seviye 2 (gelecek):** Dynamic library hot-swap (gerçek in-process reload)

### Yapılacaklar

#### 2.1 `dev_server`'de CSS dosyası ayrıştırması

**Dosya:** `crates/uwebr-cli/src/commands.rs`

`dev_server` fonksiyonunda sadece CSS değişikliğini algıla:

```rust
// Watch src/ directory
watcher.watch(root.join("src").as_path(), RecursiveMode::Recursive)?;

// Ayrı bir CSS watcher ekle (public/ veya src/ altındaki .css dosyaları)
// Veya mevcut watcher'da dosya uzantısını kontrol et

let mut css_changed = false;
let mut other_changed = false;

// Event handler'da:
for path in &event.paths {
    if path.extension().map_or(false, |e| e == "css") {
        css_changed = true;
    } else if path.extension().map_or(false, |e| e == "uwebr" || e == "rs") {
        other_changed = true;
    }
}
```

#### 2.2 CSS-only yeniden yükleme yolu

CSS değiştiğinde `cargo build` yapmadan doğrudan_stylebook'u güncelle:

**Dosya:** `crates/uwebr-cli/src/commands.rs`

```rust
if css_changed && !other_changed {
    // Sadece CSS değişti → fast path: transpile + cargo build yapmadan
    // Mevcut sürece CSS'i yeniden parse et ve yeniden başlat
    // Not: Şu an için process restart gerekiyor ama en azından
    // transpile adımı atlanıyor.
    println!("CSS changed — fast rebuild (skipping transpile)...");
    // Transpile atla, sadece cargo build
    if cargo_build(&root)? {
        if let Some(proc) = &mut app_process {
            proc.kill();
        }
        app_process = Some(AppProcess::spawn(&root, &binary)?);
    }
}
```

**Not:** Gerçek CSS-only hot reload (process restart olmadan) için uygulama tarafında `RenderPipeline`'a `reload_css()` metodu eklenmeli ve dosya watchingSpell uygulamaya sinyal göndermeli. FAZ 13'te sadece transpile atlaması yapıyoruz.

#### 2.3 RenderPipeline'a `reload_css` metodu

**Dosya:** `crates/uwebr-app/src/pipeline.rs`

```rust
impl RenderPipeline {
    /// Reload CSS without rebuilding the entire pipeline.
    ///
    /// Called by the dev server when only CSS files change.
    pub fn reload_css(&mut self, css: &str, width: u32, height: u32) {
        let _ = self.stylebook.reparse(css, width as f32, height as f32);
        self.css_string = Some(css.to_string());
    }
}
```

#### 2.4 CLI'de transpile atlama

**Dosya:** `crates/uwebr-cli/src/commands.rs`

`dev_server`'de değişiklik türüne göre dal:

```rust
// Mevcut kod:
// 1. collect changed files (debounce)
// 2. transpile_all
// 3. cargo build
// 4. kill + respawn

// Yeni:
// 1. collect changed files (debounce)
// 2. dosya tiplerini kontrol et
// 3a. sadece CSS → transpile atla, sadece cargo build
// 3b.其他 → tam döngü (transpile + cargo build)
// 4. kill + respawn
```

#### 2.5 Testler

- `commands.rs`: CSS-only değişiklik transpile'ı atlıyor (unit test)
- `pipeline.rs`: `reload_css` StyleBook'u güncelliyor
- Integration: `dev_server` mock ile test edilebilir mi? (zor, manuel test yeterli)

---

## ADIM 3: Bellek Ölçümü (~60 dk)

### Mevcut Durum

- `Metrics::measure_memory()` her zaman 0 döndürüyor
- `memory_bytes` alanı var ama dolu değil

### Platform Bağımsız Bellek Ölçümü

**Windows:** `GetProcessMemoryInfo` (psapi.dll)
**Linux:** `/proc/self/statm`
**macOS:** `task_info` (mach API)

FAZ 13'te sadece Windows desteğini ekliyoruz (mevcut geliştirme ortamı Windows).

#### 3.1 Windows bellek ölçümü

**Dosya:** `crates/uwebr-render/src/metrics.rs`

```rust
/// Best-effort resident memory estimate.
pub fn measure_memory() -> u64 {
    #[cfg(target_os = "windows")]
    {
        measure_memory_windows()
    }
    #[cfg(target_os = "linux")]
    {
        measure_memory_linux()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        0
    }
}

#[cfg(target_os = "windows")]
fn measure_memory_windows() -> u64 {
    use windows::Win32::System::ProcessStatus::GetProcessMemoryInfo;
    use windows::Win32::System::Threading::GetCurrentProcess;
    use windows::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS;

    unsafe {
        let process = GetCurrentProcess();
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(process, &mut counters, counters.cb).as_bool() {
            counters.WorkingSetSize as u64
        } else {
            0
        }
    }
}

#[cfg(target_os = "linux")]
fn measure_memory_linux() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| {
            let pages: u64 = s.split_whitespace().nth(1)?.parse().ok()?;
            let page_size = 4096u64; // standard page size
            Some(pages * page_size)
        })
        .unwrap_or(0)
}
```

#### 3.2 `windows` crate'i ekle

**Dosya:** `crates/uwebr-render/Cargo.toml`

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = ["Win32_System_ProcessStatus", "Win32_System_Threading"] }
```

**Alternatif:** `windows` crate'i ağır olabilir. Daha hafif alternatif: `sysinfo` crate'i veya raw FFI.

**Daha basit alternatif (tercih edilen):** `sysinfo` crate'i kullan:

```toml
sysinfo = "0.33"
```

```rust
use sysinfo::System;

pub fn measure_memory() -> u64 {
    let mut sys = System::new_all();
    sys.refresh_memory();
    sys.process(sysinfo::Pid::from(std::process::id() as usize))
        .map(|p| p.memory())
        .unwrap_or(0)
}
```

#### 3.3 Testler

- `metrics.rs`: `measure_memory()` Windows'ta pozitif değer döndürüyor (CI'da 0 olabilir)
- `metrics.rs`: `measure_all().memory_bytes` artık 0 değil (yerel ortamda)

---

## Sıralama

| Sıra | Adım | Karmaşıklık | Tahmini |
|------|------|-------------|---------|
| 1 | Pseudo-class/attribute selector | Yüksek | ~120 dk |
| 2 | Hot reload iyileştirmesi | Orta | ~90 dk |
| 3 | Bellek ölçümü | Orta | ~60 dk |

**Toplam tahmini:** ~270 dakika (~4.5 saat)
**Beklenen test artışı:** +14 test (7 selector + 3 hot reload + 4 memory)

---

## Riskler

1. **Selector matching karmaşıklığı:** Descendant/child selector'lar parent chain gerektirir. FAZ 13'te basitleştirilmiş (son selector eşleşmesi yeterli). Gerçek descendant matching için Element'e parent reference ekleme gerekecek.
2. **Stateful pseudo-classes:** `:hover`, `:focus` runtime state gerektirir. FAZ 13'te sadece altyapı kuruluyor, gerçek tracking sonraki fazda.
3. **Hot reload:** Gerçek in-process reload (dynamic library) çok karmaşık. FAZ 13'te transpile atlama ile ~%10-15 hızlanma sağlıyoruz.
4. **sysinfo crate'i:** `measure_memory()` her platformda çalışmayabilir (CI sandbox). 0 döndürmek kabul edilebilir.

---

## Beklenen Sonuç

- `.btn:hover { background: blue; }` → hover durumunda arka plan değişir (stateful için altyapı hazır)
- `[disabled] { opacity: 0.5; }` → disabled element soluk görünür
- `input[type="text"] { border: 1px solid; }` → sadece type="text" input'lara uygulanır
- `:first-child`, `:last-child` → basitleştirilmiş eşleşme (gerçek parent chain gelecek fazda)
- Hot reload: sadece CSS değişikliğinde transpile atlanır (~%10-15 hızlanma)
- `uwebr metrics` artık bellek bilgisini de basar (Windows/Linux)
