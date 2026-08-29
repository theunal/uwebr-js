# FAZ 15 — Structural Pseudo-Class Matching

## Amaç

`:first-child`, `:last-child`, `:nth-child(An+B)`, `:nth-of-type(An+B)`, `:empty` pseudo-class'larını gerçek CSS semantiğiyle çalışır hale getir. Mevcut stub'ları (`hep true`) kaldır.

## Değişiklik Özeti

| Dosya | Değişiklik |
|-------|-----------|
| `uwebr-css/src/ast.rs` | `PseudoClass` → `Nth(NthSelector)` variant'ı |
| `uwebr-css/src/parser.rs` | `An+B` notasyonunu parse et (discarded yerine) |
| `uwebr-render/src/stylebook.rs` | `pseudo_class_matches` gerçek child index ile eşleştir |

---

## Adım 1 — AST: `PseudoClass`'ı Zenginleştir

**Dosya:** `crates/uwebr-css/src/ast.rs`

`PseudoClass(Box<CssSelector>, String)` yerine yapısal pseudo-class'lar için `Nth` variant'ı ekle:

```rust
pub enum CssSelector {
    // ... mevcutlar ...
    /// div:hover, div:focus (stateful, argümansız)
    PseudoClass(Box<CssSelector>, String),
    /// div:nth-child(2n+1), li:first-child, ul:first-of-type
    Nth {
        selector: Box<CssSelector>,
        kind: NthKind,
        /// An+B notasyonunun ham string'i, ör. "2n+1", "3", "-n+3"
        argument: Option<String>,
    },
    /// div:not(.active) — selector listesi
    Not {
        selector: Box<CssSelector>,
        inner: Box<CssSelector>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NthKind {
    FirstChild,
    LastChild,
    FirstOfType,
    LastOfType,
    OfType,  // nth-of-type generic
}
```

---

## Adım 2 — Parser: `An+B` Notasyonunu Sakla

**Dosya:** `crates/uwebr-css/src/parser.rs`

Mevcut kod (satır 269-274):
```rust
if chars.peek() == Some(&'(') {
    let _ = read_until(chars, ')');  // ←_discarded_
    if chars.peek() == Some(&')') {
        chars.next();
    }
}
sel = CssSelector::PseudoClass(Box::new(sel), pseudo_name);
```

Yeni kod:
```rust
let mut argument = None;
if chars.peek() == Some(&'(') {
    chars.next(); // consume '('
    argument = Some(read_until(chars, ')'));
    if chars.peek() == Some(&')') {
        chars.next();
    }
}
// Yapısal pseudo-class'ları Nth variant'ına yönlendir
match pseudo_name.as_str() {
    "first-child" => {
        sel = CssSelector::Nth {
            selector: Box::new(sel),
            kind: NthKind::FirstChild,
            argument: None,
        };
    }
    "last-child" => {
        sel = CssSelector::Nth {
            selector: Box::new(sel),
            kind: NthKind::LastChild,
            argument: None,
        };
    }
    "nth-child" => {
        sel = CssSelector::Nth {
            selector: Box::new(sel),
            kind: NthKind::FirstChild,
            argument,
        };
    }
    "nth-last-child" => {
        sel = CssSelector::Nth {
            selector: Box::new(sel),
            kind: NthKind::LastChild,
            argument,
        };
    }
    "first-of-type" => {
        sel = CssSelector::Nth {
            selector: Box::new(sel),
            kind: NthKind::FirstOfType,
            argument: None,
        };
    }
    "last-of-type" => {
        sel = CssSelector::Nth {
            selector: Box::new(sel),
            kind: NthKind::LastOfType,
            argument: None,
        };
    }
    "nth-of-type" => {
        sel = CssSelector::Nth {
            selector: Box::new(sel),
            kind: NthKind::OfType,
            argument,
        };
    }
    "nth-last-of-type" => {
        sel = CssSelector::Nth {
            selector: Box::new(sel),
            kind: NthKind::LastOfType, // reuse with reversed indexing
            argument,
        };
    }
    "empty" => {
        sel = CssSelector::Nth {
            selector: Box::new(sel),
            kind: NthKind::Empty, // special: no argument, just empty check
            argument: None,
        };
    }
    _ => {
        sel = CssSelector::PseudoClass(Box::new(sel), pseudo_name);
    }
}
```

---

## Adım 3 — `parse_nth` Fonksiyonu

**Dosya:** `crates/uwebr-css/src/parser.rs` (yeni pub fonksiyon)

```rust
/// An+B notasyonunu değerlendir: "2n+1" → (a=2, b=1), "3" → (a=0, b=3), "-n+3" → (a=-1, b=3)
///
/// Returns None for invalid input.
pub fn parse_nth(arg: &str) -> Option<(i32, i32)> {
    let arg = arg.trim().to_lowercase();
    if arg == "odd" { return Some((2, 1)); }
    if arg == "even" { return Some((2, 0)); }

    // "An+B", "An-B", "An", "B", "-An+B", etc.
    let parts: Vec<&str> = arg.splitn(2, |c: char| c == '+' || c == '-');
    // tricky: negatif B needs special handling

    // Simpler approach: scan for 'n' to split A and B
    if let Some(n_pos) = arg.find('n') {
        let a_str = arg[..n_pos].trim();
        let a = if a_str.is_empty() || a_str == "+" {
            1
        } else if a_str == "-" {
            -1
        } else {
            a_str.parse::<i32>().ok()?
        };
        let rest = arg[n_pos + 1..].trim();
        let b = if rest.is_empty() {
            0
        } else if rest.starts_with('+') {
            rest[1..].trim().parse::<i32>().ok()?
        } else if rest.starts_with('-') {
            -rest[1..].trim().parse::<i32>().ok()?
        } else {
            rest.parse::<i32>().ok()?
        };
        Some((a, b))
    } else {
        // No 'n' → just B
        let b = arg.parse::<i32>().ok()?;
        Some((0, b))
    }
}
```

---

## Adım 4 — `pseudo_class_matches` Gerçek Eşleşme

**Dosya:** `crates/uwebr-render/src/stylebook.rs`

Mevcut stub kodunu kaldır ve gerçek child index hesaplamasıyla değiştir:

```rust
fn nth_matches(kind: &NthKind, argument: &Option<String>, element: &Element, parent_chain: &[&Element]) -> bool {
    let parent = match parent_chain.first() {
        Some(p) => p,
        None => return kind == &NthKind::Empty, // kök element: empty değil (kendi children'ı var)
    };

    let tag = match &element.node_type {
        NodeType::Element(t) => t.as_str(),
        _ => return false,
    };

    match kind {
        NthKind::Empty => {
            element.children.is_empty()
        }
        NthKind::FirstChild => {
            // element, parent'ın ilk çocuğu mu?
            parent.children.first().map_or(false, |first| first as *const _ == element as *const _)
        }
        NthKind::LastChild => {
            parent.children.last().map_or(false, |last| last as *const _ == element as *const _)
        }
        NthKind::FirstOfType => {
            parent.children.iter().find(|c| matches!(&c.node_type, NodeType::Element(t) if t == tag))
                .map_or(false, |first| first as *const _ == element as *const _)
        }
        NthKind::LastOfType => {
            parent.children.iter().rfind(|c| matches!(&c.node_type, NodeType::Element(t) if t == tag))
                .map_or(false, |last| last as *const _ == element as *const _)
        }
        NthKind::OfType => {
            // nth-of-type: same-type sibling index (1-based)
            match argument {
                Some(arg) => {
                    let index = parent.children.iter()
                        .filter(|c| matches!(&c.node_type, NodeType::Element(t) if t == tag))
                        .take_while(|c| *c as *const _ != element as *const _)
                        .count() + 1; // 1-based
                    match parse_nth(arg) {
                        Some((a, b)) => {
                            if a == 0 {
                                index as i32 == b
                            } else {
                                // index = a*n + b → n = (index - b) / a, check integer
                                (index as i32 - b) % a == 0 && (index as i32 - b) / a >= 0
                            }
                        }
                        None => true, // invalid arg → always match (lenient)
                    }
                }
                None => false, // no argument for nth-of-type
            }
        }
    }
}
```

---

## Adım 5 — `selector_matches` + `selector_specificity` Güncelleme

`CssSelector::Nth` ve `CssSelector::Not` için match kolları ekle:

```rust
// selector_matches:
CssSelector::Nth { selector: inner, kind, argument } => {
    selector_matches(inner, element, tag, parent_chain, node_id)
        && nth_matches(kind, argument, element, parent_chain)
}
CssSelector::Not { selector: outer, inner } => {
    selector_matches(outer, element, tag, parent_chain, node_id)
        && !selector_matches(inner, element, tag, parent_chain, node_id)
}
```

`selector_specificity`:
```rust
CssSelector::Nth { selector, .. } | CssSelector::Not { selector, .. } => {
    *classes += 1; // nth/not = 0,1,0 specificity (class level)
    count(selector, ids, classes, tags);
}
```

---

## Adım 6 — Codegen Güncellemesi

`crates/uwebr-css/src/codegen.rs` — `CssSelector::Nth` ve `CssSelector::Not` için render:

```rust
CssSelector::Nth { selector, kind, argument } => {
    let base = render_selector(selector);
    match kind {
        NthKind::FirstChild => format!("{base}:first-child"),
        NthKind::LastChild => format!("{base}:last-child"),
        NthKind::FirstOfType => format!("{base}:first-of-type"),
        NthKind::LastOfType => format!("{base}:last-of-type"),
        NthKind::OfType => {
            let arg = argument.as_deref().unwrap_or("0");
            format!("{base}:nth-of-type({arg})")
        }
        NthKind::Empty => format!("{base}:empty"),
    }
}
CssSelector::Not { selector, inner } => {
    let base = render_selector(selector);
    let inner_sel = render_selector(inner);
    format!("{base}:not({inner_sel})")
}
```

---

## Adım 7 — Testler

1. `test_nth_child_an_plus_b` — `:nth-child(2n+1)` 1., 3., 5. çocuklarda eşleşir
2. `test_nth_child_odd_even` — `:odd` / `:even` notasyonu
3. `test_nth_child_no_argument` — `:nth-child(3)` sadece 3. çocuk
4. `test_last_child` — son çocuk eşleşir, diğerleri eşleşmez
5. `test_first_of_type` — tag_name'e göre ilk çocuk
6. `test_last_of_type` — tag_name'e göre son çocuk
7. `test_nth_of_type` — `:nth-of-type(2)` tag-filtreli index
8. `test_empty_matches_no_children` — boş element eşleşir
9. `test_empty_no_match_with_children` — çocuğu olan element eşleşmez
10. `test_not_selector` — `div:not(.active)` negatif eşleşme
11. `test_parse_nth_odd` — "odd" → (2, 1)
12. `test_parse_nth_even` — "even" → (2, 0)
13. `test_parse_nth_simple` — "3" → (0, 3)
14. `test_parse_nth_complex` — "2n+1" → (2, 1), "-n+3" → (-1, 3)
15. `test_nth_child_specificity` — specificity: (0, 1, 0) class level

---

## Uygulama Sırası

1. `ast.rs` → Nth + NthKind + Not variant'ları
2. `parser.rs` → parse_nth fonksiyonu + pseudo-class yönlendirme
3. `codegen.rs` → Nth + Not render
4. `stylebook.rs` → nth_matches + selector_matches + selector_specificity + pseudo_class_matches
5. Testleri yaz ve doğrula
6. `cargo clippy --workspace` → 0 uyarı
7. `cargo test --workspace` → tüm testler yeşil
8. `ARCHITECTURE.md` güncelle

## Beklenen Sonuç

- **Test sayısı:** ~554 (+15 yeni test)
- **Clippy:** 0 uyarı
- `li:first-child { font-weight: bold; }` sadece ilk `<li>`'ye uygulanır
- `tr:nth-child(even) { background: #f5f5f5; }` çift satırlara uygulanır
- `:empty { display: none; }` çocuğu olmayan element'leri gizler
- `div:not(.active) { opacity: 0.5; }` sadece `.active` olmayan div'lere uygulanır
