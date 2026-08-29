# FAZ 9: Component Props Entegrasyonu

> Durum: 📋 Plan hazır, onay bekliyor
> Oluşturma: 29 Ağustos 2026

## Problemin Tanımı

`<Card title="Hello" disabled />` yazıldığında transpiler şunu üretir:

```rust
Element {
    node_type: NodeType::Component("Card".into()),
    props: vec![
        ("title".into(), PropValue::String("Hello".into())),
        ("disabled".into(), PropValue::Bool(true)),
    ],
    children: vec![card_component()],  // ← sıfır argüman!
}
```

`card_component()` hiç argüman almıyor. Props Element'e yazılıyor ama component fonksiyonuna ulaşmıyor.

## Çözüm Stratejisi

Runtime props bridge — transpiler props'u component fonksiyonuna geçirir.

---

## Adım 1: Helper Fonksiyonları Ekle

**Dosya:** `crates/uwebr-core/src/component.rs`

Props'tan değer okuma helper fonksiyonları:

```rust
pub fn prop_string(props: &[(String, PropValue)], key: &str) -> String
pub fn prop_bool(props: &[(String, PropValue)], key: &str) -> bool
pub fn prop_number(props: &[(String, PropValue)], key: &str) -> f64
```

---

## Adım 2: Transpiler'da Props Geçişi

**Dosya:** `crates/uwebr-cli/src/transpiler.rs` (satır 435-473)

`HtmlNode::Component` kolunda component fonksiyonuna props geçir:

```rust
// Mevcut:
format!("vec![{}()]", fn_name)

// Yeni — props varsa:
format!("vec![{}(&[{}])]", fn_name, props.join(", "))
```

---

## Adım 3: Component Fonksiyon İmzasını Güncelle

**Dosya:** `crates/uwebr-cli/src/transpiler.rs`

Component fonksiyonu artık `__props: &[(String, PropValue)]` alsın:

```rust
// Mevcut:
fn header_component() -> Element { ... }

// Yeni:
fn header_component(__props: &[(String, PropValue)]) -> Element {
    let title = prop_string(__props, "title");
    let disabled = prop_bool(__props, "disabled");
    Element { ... }
}
```

---

## Adım 4: Import'ları Ekle

**Dosya:** `crates/uwebr-cli/src/transpiler.rs`

Component fonksiyonu üretilirken:

```rust
use uwebr_core::component::{prop_string, prop_bool, prop_number, PropValue};
```

---

## Adım 5: Testler

### 5.1 Unit Test — `uwebr-core/src/component.rs`

- `test_prop_string` — String prop okuma
- `test_prop_bool` — Bool prop okuma
- `test_prop_number` — Number prop okuma

### 5.2 Transpiler Test — `uwebr-cli/src/transpiler.rs`

- `test_component_receives_props` — Props olan component `&[...]` ile çağrılır
- `test_component_no_props_uses_zero_args` — Props olmayan component `()` ile çağrılır

### 5.3 Entegrasyon Test — `uwebr-core/tests/`

- `test_props_flow_to_component` — Props'un component fonksiyonuna aktarılması

---

## Değişiklik Özeti

| Dosya | Değişiklik | Tahmini Satır |
|-------|-----------|---------------|
| `uwebr-core/src/component.rs` | Helper fonksiyonları | +30 |
| `uwebr-cli/src/transpiler.rs` | Props geçir + imza + import | ~18 |
| Test dosyaları | Yeni testler | +60 |

**Toplam:** ~50 satır kod değişikliği + ~60 satır test

---

## Sıralama

| Adım | İşlem | Tahmini |
|------|-------|---------|
| 1 | Helper fonksiyonları ekle | 10 dk |
| 2 | Transpiler'da props geçir | 15 dk |
| 3 | Component fonksiyon imzasını güncelle | 15 dk |
| 4 | Import'ları ekle | 5 dk |
| 5 | Testleri yaz | 20 dk |
| 6 | `cargo test --workspace` | 5 dk |
| 7 | Clippy + fmt | 5 dk |

**Toplam tahmini:** ~75 dakika

---

## Beklenen Sonuç

- `<Card title="Hello" />` → `card_component(&[("title".into(), PropValue::String("Hello".into()))])`
- Props component fonksiyonunda okunabilir
- Eski davranış korunur (props olmayan component'ler `()` ile çağrılır)
- Test sayısı artar (~+10 test)


FAZ 9: Component Props Entegrasyonu — Detaylı Plan
Problemin Tanımı
Şu an <Card title="Hello" disabled /> yazıldığında transpiler şunu üretiyor:
Element {
    node_type: NodeType::Component("Card".into()),
    props: vec![
        ("title".into(), PropValue::String("Hello".into())),
        ("disabled".into(), PropValue::Bool(true)),
    ],
    children: vec![card_component()],  // ← sıfır argüman!
}
card_component() hiç argüman almıyor. Props Element'e yazılıyor ama component fonksiyonuna ulaşmıyor.
Çözüm Stratejisi: İki Aşamalı Yaklaşım
Aşama 1 (Bu faz): Runtime props bridge — transpiler props'u component fonksiyonuna geçirir
Aşama 2 (Gelecek faz): Macro-generated typed props — #[component] + #[derive(Props)] entegrasyonu
Adım 1: ComponentFn Tipini Genişlet
Dosya: crates/uwebr-core/src/component.rs
// Mevcut:
pub type ComponentFn = fn() -> Element;

// Yeni:
pub type ComponentFn = fn() -> Element;
pub type PropsComponentFn = fn(&[(String, PropValue)]) -> Element;
Neden: Eski API korunur, yeni variant eklenir. FnComponent iki tibi de destekler.
Ek değişiklik: FnComponent struct'ına optional props parametresi:
pub struct FnComponent {
    render_fn: Box<dyn Fn() -> Element + Send + 'static>,
    props_fn: Option<Box<dyn Fn(&[(String, PropValue)]) -> Element + Send + 'static>>,
}
Adım 2: Transpiler'da Props Geçişi
Dosya: crates/uwebr-cli/src/transpiler.rs (satır 435-473)
Değişiklik: HtmlNode::Component kolunda component fonksiyonuna props geçir:
// Mevcut (satır 458-459):
format!("vec![{}()]", fn_name)

// Yeni:
if props.is_empty() {
    format!("vec![{}()]", fn_name)
} else {
    format!("vec![{}(&[{}])]", fn_name, props.join(", "))
}
Slot children kısmında da aynı mantık:
// Mevcut (satır 462):
format!("{{ let mut __c = vec![{}()]; __c.extend(...); __c }}", fn_name)

// Yeni:
format!("{{ let mut __c = vec![{}(&[{}])]; __c.extend(...); __c }}", fn_name, props_str)
Adım 3: Component Fonksiyon İmzasını Güncelle
Dosya: Transpiler — component fonksiyonu üretimi (satır ~170-190)
Şu an transpiler her component için fn xxx_component() -> Element üretiyor. Yeni üretim:
// Mevcut:
fn header_component() -> Element { ... }

// Yeni — props alan versiyon:
fn header_component(__props: &[(String, PropValue)]) -> Element {
    // Props'tan değer okuma helper'ları:
    fn prop_string(props: &[(String, PropValue)], key: &str) -> String {
        props.iter().find_map(|(k, v)| {
            if k == key { match v { PropValue::String(s) => Some(s.clone()), _ => None } } else { None }
        }).unwrap_or_default()
    }
    fn prop_bool(props: &[(String, PropValue)], key: &str) -> bool {
        props.iter().find_map(|(k, v)| {
            if k == key { match v { PropValue::Bool(b) => Some(*b), _ => None } } else { None }
        }).unwrap_or(false)
    }
    fn prop_number(props: &[(String, PropValue)], key: &str) -> f64 {
        props.iter().find_map(|(k, v)| {
            if k == key { match v { PropValue::Number(n) => Some(*n), PropValue::String(s) => s.parse().ok(), _ => None } } else { None }
        }).unwrap_or(0.0)
    }
    
    // Component gövdesi — props'lara erişim:
    let title = prop_string(__props, "title");
    let disabled = prop_bool(__props, "disabled");
    
    Element { ... }
}
Önemli: Props helper fonksiyonları her component başında üretilmeli (veya uwebr-core'a ortak helper olarak eklenebilir).
Adım 4: Helper Fonksiyonları uwebr-core'a Ekle
Dosya: crates/uwebr-core/src/component.rs
/// Props'tan String değeri oku
pub fn prop_string(props: &[(String, PropValue)], key: &str) -> String {
    props.iter().find_map(|(k, v)| {
        if k == key {
            match v {
                PropValue::String(s) => Some(s.clone()),
                _ => None,
            }
        } else {
            None
        }
    }).unwrap_or_default()
}

/// Props'tan bool değeri oku
pub fn prop_bool(props: &[(String, PropValue)], key: &str) -> bool {
    props.iter().find_map(|(k, v)| {
        if k == key {
            match v {
                PropValue::Bool(b) => Some(*b),
                _ => None,
            }
        } else {
            None
        }
    }).unwrap_or(false)
}

/// Props'tan Number değeri oku (String'den de parse eder)
pub fn prop_number(props: &[(String, PropValue)], key: &str) -> f64 {
    props.iter().find_map(|(k, v)| {
        if k == key {
            match v {
                PropValue::Number(n) => Some(*n),
                PropValue::String(s) => s.parse().ok(),
                _ => None,
            }
        } else {
            None
        }
    }).unwrap_or(0.0)
}
Adım 5: Transpiler'da Helper Import'u
Dosya: crates/uwebr-cli/src/transpiler.rs
Component fonksiyonu üretilirken use ifadesi ekle:
use uwebr_core::component::{prop_string, prop_bool, prop_number, PropValue};
Bu import component fonksiyonunun başında olmalı.
Adım 6: Macro Entegrasyonu (Opsiyonel Aşama)
Dosya: crates/uwebr-macro/src/lib.rs
#[component] macro'sunu güncelle — artık __props: &[(String, PropValue)] alsın:
#[proc_macro_attribute]
pub fn component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_vis = &input_fn.vis;
    let fn_block = &input_fn.block;
    let fn_sig = &input_fn.sig;
    let fn_attrs = &input_fn.attrs;

    // Fonksiyon imzasını __props parametresiyle genişlet
    let expanded = quote! {
        #(#fn_attrs)*
        #fn_vis #fn_sig {
            let __cid = ::uwebr_core::lifecycle::create_component_scope();
            ::uwebr_core::lifecycle::with_component(__cid, || {
                ::uwebr_core::lifecycle::on_cleanup(move || {
                    ::uwebr_core::lifecycle::trigger_cleanup(__cid);
                });
                let __r = #fn_block;
                ::uwebr_core::lifecycle::trigger_mount(__cid);
                __r
            })
        }
    };
    TokenStream::from(expanded)
}
Not: Macro'nun mevcut davranışı korunur. Fonksiyon zaten __props alıyorsa lifecycle wrapper'ı çalışır. Bu adım opsiyoneldir — temel props geçişi macro olmadan da çalışır.
Adım 7: Testler
7.1 Unit Test — uwebr-core/src/component.rs
#[test]
fn test_prop_string() {
    let props = vec![
        ("title".into(), PropValue::String("Hello".into())),
        ("disabled".into(), PropValue::Bool(true)),
    ];
    assert_eq!(prop_string(&props, "title"), "Hello");
    assert_eq!(prop_string(&props, "missing"), "");
}

#[test]
fn test_prop_bool() {
    let props = vec![("disabled".into(), PropValue::Bool(true))];
    assert!(prop_bool(&props, "disabled"));
    assert!(!prop_bool(&props, "missing"));
}

#[test]
fn test_prop_number() {
    let props = vec![("count".into(), PropValue::Number(42.0))];
    assert_eq!(prop_number(&props, "count"), 42.0);
}
7.2 Transpiler Test — uwebr-cli/src/transpiler.rs
#[test]
fn test_component_receives_props() {
    let input = r#"
        <div><Card title="Hello" count={42} /></div>
        <script></script>
        <style></style>
    "#;
    let result = transpile(input, "App").unwrap();
    assert!(result.contains("card_component(&["));
    assert!(result.contains("PropValue::String(\"Hello\"".into()));
    assert!(result.contains("PropValue::String((42).to_string())"));
}

#[test]
fn test_component_no_props_uses_zero_args() {
    let input = r#"
        <div><Card /></div>
        <script></script>
        <style></style>
    "#;
    let result = transpile(input, "App").unwrap();
    assert!(result.contains("card_component()"));
}
7.3 Entegrasyon Test — uwebr-core/tests/
#[test]
fn test_props_flow_to_component() {
    // Props'u olan bir component oluştur ve test et
    fn card_component(props: &[(String, PropValue)]) -> Element {
        let title = prop_string(props, "title");
        Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![Element::text(&title)],
        }
    }
    
    let props = vec![("title".into(), PropValue::String("Test".into()))];
    let el = card_component(&props);
    assert_eq!(el.children[0].node_type, NodeType::Text("Test".into()));
}
Değişiklik Özeti
Dosya	Değişiklik	Satır
uwebr-core/src/component.rs	prop_string, prop_bool, prop_number helper'ları	+30 satır
uwebr-cli/src/transpiler.rs	Component çağrısına props geçir	~5 satır değişiklik
uwebr-cli/src/transpiler.rs	Component fonksiyonuna __props parametresi ekle	~10 satır
uwebr-cli/src/transpiler.rs	Helper import ekle	~3 satır
uwebr-macro/src/lib.rs	(Opsiyonel) Macro güncellemesi	~5 satır
Toplam: ~50 satır kod değişikliği + ~60 satır test
Sıralama
Adım	İşlem	Tahmini
1	Helper fonksiyonları ekle (component.rs)	10 dk
2	Transpiler'da props geçir	15 dk
3	Component fonksiyon imzasını güncelle	15 dk
4	Import'ları ekle	5 dk
5	Testleri yaz	20 dk
6	cargo test --workspace	5 dk
7	Clippy + fmt	5 dk
Toplam tahmini: ~75 dakika