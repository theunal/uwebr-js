# FAZ 14 — CSS Selector Matching: Hover/Focus State + Parent Chain + !important

## Amaç

CSS selector matching'i tam çalışır hale getir:
- `:hover` / `:focus` / `:active` pseudo-class'ları runtime state ile eşleşsin
- Descendant/Child combinators (`div > .btn`, `.nav .item`) gerçek parent chain ile eşleşsin
- `!important` declaration'ları cascade'de düşük specificity'yi yenilsin

## Değişiklik Özeti

| Dosya | Değişiklik |
|-------|-----------|
| `uwebr-core/src/state.rs` | `ElementStateStore` — hover/focus set |
| `uwebr-render/src/stylebook.rs` | `match_full` + `selector_matches` parent chain + state parametresi |
| `uwebr-render/src/layout.rs` | `build_node` parent chain geçişi |
| `uwebr-render/src/paint.rs` | `MatchedStyle.important_flags` field'ı |
| `uwebr-app/src/pipeline.rs` | `RenderPipeline` hover state management |
| `uwebr-app/src/app.rs` | CursorMoved → hover state güncelleme |
| `ARCHITECTURE.md` | Sınırlar tablosu güncelleme |

---

## Adım 1 — Element State Store (`uwebr-core`)

**Dosya:** `crates/uwebr-core/src/state.rs`

Mevcut `thread_local! { STATES }` yapısının yanına yeni bir thread-local store ekle:

```rust
use std::collections::HashSet;

thread_local! {
    static ELEMENT_STATE: RefCell<ElementStateStore> = RefCell::new(ElementStateStore::new());
}

pub struct ElementStateStore {
    /// Element node ID'leri (Element tree'deki index) hover durumunda
    hovered: HashSet<usize>,
    /// Odaklanmış element
    focused: Option<usize>,
}
```

**Public API:**
- `set_hovered(node_id: usize, hovered: bool)` — cursor hareket ettikçe çağrılır
- `set_focused(node_id: Option<usize>)` — focus/blur olaylarında çağrılır
- `is_hovered(node_id: usize) -> bool`
- `is_focused(node_id: usize) -> bool`
- `clear_all()` — her render döngüsü başında (odak hariç)

---

## Adım 2 — Selector Matching'e Parent Chain + State Ekle

**Dosya:** `crates/uwebr-render/src/stylebook.rs`

### 2a. `match_full` imzasını genişlet

```rust
// Eski:
pub fn match_full(&self, element: &Element) -> MatchedStyle

// Yeni:
pub fn match_full(&self, element: &Element, parent_chain: &[&Element]) -> MatchedStyle
```

`parent_chain`[0] = immediate parent, [1] = grandparent, ...

### 2b. `selector_matches` imzasını genişlet

```rust
// Eski:
fn selector_matches(sel: &CssSelector, element: &Element, tag: &str) -> bool

// Yeni:
fn selector_matches(
    sel: &CssSelector,
    element: &Element,
    tag: &str,
    parent_chain: &[&Element],
) -> bool
```

Descendant/Child case'leri artık gerçek kontrol yapar:

```rust
CssSelector::Descendant(selectors) => {
    // selectors = [ancestor, ..., subject]
    // subject = selectors.last() (element'e uygulanır)
    // selector_matches(element) && parent_chain'de herhangi bir ancestor eşleşmeli
    let subject = selectors.last().unwrap();
    if !selector_matches(subject, element, tag, parent_chain) {
        return false;
    }
    let ancestors = &selectors[..selectors.len() - 1];
    // parent_chain'de sola doğru tara
    let mut depth = 0;
    for ancestor_sel in ancestors.iter().rev() {
        if depth >= parent_chain.len() { return false; }
        let ancestor = parent_chain[depth];
        let a_tag = match &ancestor.node_type {
            NodeType::Element(t) => t.as_str(),
            _ => return false,
        };
        if !selector_matches(ancestor_sel, ancestor, a_tag, &parent_chain[depth+1..]) {
            return false;
        }
        depth += 1;
    }
    true
}
```

Child combinator (`A > B`): sadece bir üst parent'a bakar.

### 2c. `pseudo_class_matches` — state parametresi ekle

```rust
// Eski:
fn pseudo_class_matches(pseudo: &str, element: &Element) -> bool

// Yeni:
fn pseudo_class_matches(pseudo: &str, element: &Element, node_id: usize) -> bool
```

```rust
"hover" => uwebr_core::state::is_hovered(node_id),
"focus" | "focus-visible" => uwebr_core::state::is_focused(node_id),
"active" => false, // :active = tıklama anı (mousedown state ile aynı döngüde)
"visited" => false, // link ziyaret_state'i yok (desktop uygulaması)
"focus-within" => {
    // parent_chain'de herhangi bir element focused olmalı
    parent_chain.iter().any(|p| uwebr_core::state::is_focused_by_element(p))
}
```

### 2d. `selector_specificity` — parent chain'e dokunmaz (zaten doğru)

---

## Adım 3 — `!important` Cascade

**Dosya:** `crates/uwebr-render/src/stylebook.rs`

### 3a. `MatchedStyle`'a `important_flags` ekle

```rust
pub struct MatchedStyle {
    pub style: TaffyStyle,
    pub mask: StyleMask,
    pub paint: CssPaint,
    /// Bitset: hangi property'ler !important ile yazıldı
    pub important_flags: u64,
}
```

### 3b. `match_full`'de !important sıralaması

Mevcut sıralama: `(specificity, index)` ASC.

Yeni sıralama: `(has_important, specificity, index)` ASC.

Önce !important olanlar (ama düşük specificity bile olsa), sonra yüksek specificity:
```rust
let is_important = entry.properties.iter().any(|p| p.important);
// sıralama anahtarı: (is_important as u8, specificity, index)
```

Eşitlik durumunda: !important olan her zaman kazanır (CSS spec'e göre).

---

## Adım 4 — Layout Engine Parent Chain

**Dosya:** `crates/uwebr-render/src/layout.rs`

`build_node` recursive fonksiyonuna `parent_chain: Vec<&Element>` parametresi ekle:

```rust
fn build_node(
    &mut self,
    element: &Element,
    stylebook: &StyleBook,
    inherited: &ResolvedPaint,
    parent_chain: &[&Element],  // YENİ
) -> anyhow::Result<taffy::NodeId> {
    let matched = stylebook.match_full(element, parent_chain);
    let paint = ResolvedPaint::resolve(inherited, &matched.paint, element);
    // ... mevcut kod ...

    // children çağrısında parent_chain'i güncelle
    let mut child_chain = parent_chain.to_vec();
    child_chain.insert(0, element); // immediate parent en başa

    for child in &element.children {
        self.build_node(child, stylebook, &paint, &child_chain)?;
    }
}
```

Public `build_tree` fonksiyonu boş parent_chain ile başlar.

---

## Adım 5 — Pipeline Hover State Management

**Dosya:** `crates/uwebr-app/src/pipeline.rs`

### 5a. `RenderPipeline`'a `element_positions` ekle

```rust
pub struct RenderPipeline {
    // ... mevcut alanlar ...
    /// Layout sonrası element pozisyonları (hit-test + hover için)
    element_positions: Vec<(usize, LayoutInfo)>,
    /// Son build'den bu yana hover değişti mi
    hover_dirty: bool,
}
```

### 5b. `build_render_scene` sonunda pozisyonları kaydet

Layout完成后 `positioned_nodes`'i element pozisyonlarıyla eşle:

```rust
pub fn build_render_scene(...) {
    let positioned = self.layout_engine.build_tree(element, &self.stylebook, ...);
    self.element_positions = self.collect_element_positions(element, &positioned);
    // ...
}
```

### 5c. `hit_test_hover` metodu

```rust
pub fn hit_test_hover(&self, x: f32, y: f32) -> Option<usize> {
    self.element_positions.iter()
        .filter(|(_, pos)| pos.x <= x && x <= pos.x + pos.width
                         && pos.y <= y && y <= pos.y + pos.height)
        .max_by_key(|(_, pos)| pos.depth)
        .map(|(id, _)| *id)
}
```

### 5d. `reload_css` → `needs_full_rebuild` flag

CSS değiştiğinde layout'u da yeniden hesapla (sadece stylebook swap yetmez):
```rust
pub fn reload_css(&mut self, css: &str, width: u32, height: u32) {
    let _ = self.stylebook.reparse(css, width as f32, height as f32);
    self.css_string = Some(css.to_string());
    self.needs_full_rebuild = true;  // YENİ
}
```

---

## Adım 6 — App'te Cursor → Hover State

**Dosya:** `crates/uwebr-app/src/app.rs`

`handle_cursor_moved` metodunu güncelle:

```rust
fn handle_cursor_moved(&mut self, window_id: WindowId, x: f32, y: f32) {
    let state = self.windows.get_mut(&window_id).unwrap();
    state.cursor = (x, y);

    // Hover state güncelle
    if let Some(new_hovered) = state.pipeline.hit_test_hover(x, y) {
        let old_hovered = state.hovered_element;
        if old_hovered != new_hovered {
            // Eski hover'dan çıkar
            if let Some(old) = old_hovered {
                uwebr_core::state::set_hovered(old, false);
            }
            // Yeni hover'a gir
            uwebr_core::state::set_hovered(new_hovered, true);
            state.hovered_element = Some(new_hovered);
            state.pipeline.mark_hover_dirty(); // re-render tetikle
        }
    }
}
```

---

## Adım 7 — Testler

Yeni testler (her adım için):

1. **`test_element_state_hover`** — `set_hovered` / `is_hovered` round-trip
2. **`test_element_state_focus`** — `set_focused` / `is_focused` round-trip
3. **`test_pseudo_class_hover_matches`** — `:hover` pseudo-class doğru element'e uygulanıyor mu
4. **`test_pseudo_class_focus_matches`** — `:focus` pseudo-class doğru element'e uygulanıyor mu
5. **`test_descendant_selector_real_match`** — `.parent .child` sadece gerçek child'larda eşleşir
6. **`test_child_selector_direct_only`** — `div > .btn` sadece doğrudan child'larda eşleşir
7. **`test_descendant_no_match_nested_wrong`** — `.unrelated .child` yanlış parent'da eşleşmez
8. **`test_important_wins_over_higher_specificity`** — `.a { color: red !important }` > `#id { color: blue }`
9. **`test_important_equal_specificity`** — İki !important kural 같은 specificity → son kazanır
10. **`test_hover_triggers_rerender`** — Hover state değişimi re-render tetikler
11. **`test_parent_chain_passed_to_match`** — `match_full`'a doğru parent chain geçiriliyor

---

## Uygulama Sırası

1. `uwebr-core/src/state.rs` → ElementStateStore (bağımsız, bağımlılığı yok)
2. `uwebr-render/src/stylebook.rs` → selector_matches parent chain + pseudo_class_matches state
3. `uwebr-render/src/layout.rs` → build_node parent chain geçişi
4. `uwebr-render/src/paint.rs` → MatchedStyle.important_flags (opsiyonel, basit)
5. `uwebr-app/src/pipeline.rs` → element_positions + hit_test_hover
6. `uwebr-app/src/app.rs` → cursor → hover state
7. Testleri yaz ve doğrula
8. `cargo clippy --workspace` → 0 uyarı
9. `cargo test --workspace` → tüm testler yeşil
10. `ARCHITECTURE.md` güncelle

## Beklenen Sonuç

- **Test sayısı:** ~534 (+11 yeni test)
- **Clippy:** 0 uyarı
- `.parent .child { ... }` artık sadece gerçek child'larda çalışır
- `button:hover { background: blue; }` element üzerinde hover iken uygulanır
- `input:focus { border: 2px solid blue; }` odaklanınca uygulanır
- `!important` düşük specificity'yi yener
