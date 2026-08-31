# CSS Advanced Features Implementation Plan

## Overview

4 major features, implemented in dependency order. Each feature is a
self-contained faz (phase) with its own commit.

---

## Faz 1: CSS Variables (`var()`) + `calc()`

### Goal
Support `--my-color: red` custom properties and `var(--my-color)` / `calc(100% - 20px)`.

### Architecture Insight
- `CssValue` enum in `ast.rs` is the foundation — all property values flow through it.
- Variable resolution must happen BEFORE `extract_paint()` / `extract_layout()` consume properties.
- Two possible strategies: (a) resolve at parse time, (b) resolve at style-match time.
- **Chosen: resolve at style-match time** — variables are per-element (inheritable), so they
  need the element context to resolve correctly.

### Files to Modify

#### 1. `crates/uwebr-css/src/ast.rs`
- Add `CssValue::Var { name: String, fallback: Option<Box<CssValue>> }` variant.
- Add `CssValue::Calc(Vec<CalcToken>)` variant for `calc()` expressions.
- Add `CalcToken` enum:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CalcToken {
    Value(f32),           // resolved numeric value
    Add,
    Sub,
    Mul,
    Div,
    OpenParen,
    CloseParen,
    Length(f32, LengthUnit), // e.g. 100%
}
```

- Add `CssProperty::CustomProperty { name: String, value: CssValue }` variant
  OR reuse existing `CssProperty { name: "--my-color", value }` (name starts with `--`).

#### 2. `crates/uwebr-css/src/parser.rs`
- In `parse_value()` / `parse_single_value()`:
  - Detect `var(` token → parse `var(--name)` or `var(--name, fallback)`.
  - Detect `calc(` token → parse arithmetic expression into `CalcToken` list.
- Variable reference parsing: `var(--my-color, #fff)` → `CssValue::Var { name, fallback }`.
- Calc parsing: `calc(100% - 20px)` → tokenize into `CalcToken` list with precedence.

#### 3. `crates/uwebr-css/src/codegen.rs`
- Add `resolve_custom_properties(rules: &mut [CssRule], context: &CustomPropertyMap)` function.
- Add `resolve_var(value: &CssValue, context: &CustomPropertyMap) -> CssValue` — walks the value tree and replaces `Var` nodes.
- Add `eval_calc(tokens: &[CalcToken], context: &CustomPropertyMap) -> Option<f32>` — evaluates arithmetic.
- `CustomPropertyMap = HashMap<String, CssValue>` — collected from `--*` properties.
- In `convert_to_style_entries_vp()`: before processing each rule, collect `--*` properties
  from the rule's own properties into the custom property map, then resolve `var()` / `calc()`.

#### 4. `crates/uwebr-render/src/layout.rs`
- In `build_tree()`: when matching a rule's properties, the custom property map is inherited
  from parent → child (like CSS custom properties inheritance).
- Store `custom_properties: HashMap<String, CssValue>` on the parent chain context.

#### 5. `crates/uwebr-render/src/stylebook.rs`
- No changes needed — variable resolution happens in codegen before style entries are built.
  But for per-element variable inheritance, `match_full()` needs access to the element's
  custom property map.

### Detailed Flow

```
CSS: ".root { --color: red; } .child { color: var(--color); width: calc(100% - 20px); }"
  ↓
parse_css() → [CssRule ".root" with (--color, red), CssRule ".child" with (color, var(--color)), ...]
  ↓
convert_to_style_entries_vp():
  For each rule:
    1. Collect --* properties into rule's custom_properties map
    2. For each non-custom property:
       a. If value is Var → resolve using rule's + ancestor custom properties
       b. If value contains Var in sub-expressions → resolve recursively
       c. If value is Calc → evaluate arithmetic (needs resolved lengths)
    3. Build StyleEntry with resolved values
```

### Tests (12+ tests)
- `test_var_simple`: `--color: red` → `color: var(--color)` resolves to red.
- `test_var_fallback`: `var(--missing, blue)` → resolves to blue.
- `test_var_nested`: `var(--a, var(--b, 10px))` → resolves inner fallback.
- `test_var_inherited`: parent defines `--x`, child uses `var(--x)`.
- `test_calc_basic`: `calc(100% - 20px)` → evaluates to width minus 20.
- `test_calc_multiply`: `calc(2 * 50px)` → 100px.
- `test_calc_nested`: `calc(100% - calc(10px + 5px))`.
- `test_calc_with_var`: `calc(100% - var(--margin))`.
- `test_custom_property_not_leaked`: `--color` doesn't affect paint/layout.
- `test_var_in_gradient`: `var()` inside gradient stops.

---

## Faz 2: `@media` Query Evaluation

### Goal
Evaluate `@media (max-width: 768px)`, `@media (min-height: 400px)`, `@media (orientation: portrait)`.

### Architecture Insight
- `CssRule.media_query: Option<String>` already stores the raw string.
- `StyleBook::reparse(css, vw, vh)` is called on window resize with new viewport dims.
- Media evaluation should filter rules during `convert_to_style_entries_vp()`.

### Files to Modify

#### 1. `crates/uwebr-css/src/ast.rs`
- Add `MediaQuery` struct:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaQuery {
    pub conditions: Vec<MediaCondition>,
    pub feature: String,  // "width", "height", "min-width", "max-width", etc.
    pub value: MediaValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MediaValue {
    Length(f32, LengthUnit),
    Keyword(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaCondition {
    pub negated: bool,
    pub feature: String,
    pub value: MediaValue,
}
```

- Add `CssRule.media_conditions: Option<Vec<MediaCondition>>` (parsed form of `media_query`).

#### 2. `crates/uwebr-css/src/parser.rs`
- Add `parse_media_query(query: &str) -> Vec<MediaCondition>` function.
- Parse `(max-width: 768px)` → `MediaCondition { feature: "max-width", value: Length(768.0, Px) }`.
- Parse `(orientation: portrait)` → `MediaCondition { feature: "orientation", value: Keyword("portrait") }`.
- Handle `and` / `or` / `not` / comma-separated queries.
- Called from `parse_at_rule()` when handling `@media`.

#### 3. `crates/uwebr-css/src/codegen.rs`
- Add `media_matches(condition: &MediaCondition, vw: f32, vh: f32) -> bool` function.
  - `min-width` / `max-width` → compare against `vw`.
  - `min-height` / `max-height` → compare against `vh`.
  - `orientation` → portrait if vh > vw, landscape otherwise.
  - `prefers-color-scheme` → always "light" (desktop app).
  - `print` → always false.
- Add `rule_media_matches(rule: &CssRule, vw: f32, vh: f32) -> bool` — evaluates all conditions.
- In `convert_to_style_entries_vp()`: skip rules where `rule_media_matches()` returns false.

#### 4. `crates/uwebr-render/src/stylebook.rs`
- In `parse_vp()` / `reparse()`: pass `vw`, `vh` through to codegen so media queries resolve.

### Tests (8+ tests)
- `test_media_max_width_match`: `@media (max-width: 768px)` with vw=400 → matches.
- `test_media_max_width_no_match`: with vw=1024 → doesn't match.
- `test_media_min_height`: `@media (min-height: 600px)` with vh=400 → doesn't match.
- `test_media_orientation_portrait`: portrait → matches portrait query.
- `test_media_and_conditions`: `@media (min-width: 320px) and (max-width: 768px)` → both must match.
- `test_media_not`: `@media not print` → always matches.
- `test_media_comma`: `@media (max-width: 768px), (max-height: 400px)` → either matches.
- `test_media_filter_rules`: rules with non-matching media queries are excluded from StyleBook.

---

## Faz 3: Pseudo-elements + Sibling Combinators

### Goal
- `::before` / `::after` synthetic content injection.
- `h1 + p` (adjacent sibling), `h1 ~ p` (general sibling).

### Architecture Insight
- `CssSelector` enum has no `PseudoElement`, `AdjacentSibling`, or `GeneralSibling` variants.
- `selector_matches()` in stylebook.rs is the matching engine — it needs new arms.
- `::before`/`::after` need synthetic Element creation with `content` property.
- Sibling matching needs the parent's children list — currently `parent_chain` only has ancestors.

### Files to Modify

#### 1. `crates/uwebr-css/src/ast.rs`
- Add selector variants:

```rust
pub enum CssSelector {
    // ... existing ...
    PseudoElement {
        selector: Box<CssSelector>,
        name: String,  // "before", "after"
    },
    AdjacentSibling(Vec<CssSelector>),  // A + B
    GeneralSibling(Vec<CssSelector>),   // A ~ B
}
```

#### 2. `crates/uwebr-css/src/parser.rs`
- In `parse_selector()`:
  - Detect `::` → parse pseudo-element name → wrap selector in `PseudoElement`.
  - Detect `+` → parse next selector → wrap in `AdjacentSibling`.
  - Detect `~` → parse next selector → wrap in `GeneralSibling`.
- Priority: `::before`/`::after` must be parsed after class/id but before combinators.

#### 3. `crates/uwebr-render/src/stylebook.rs`
- Add `PseudoElement` arm in `selector_matches()`:
  - Check if the element is a synthetic `::before`/`::after` node.
  - Match the inner selector against the parent element.
- Add `AdjacentSibling` arm:
  - Given `A + B`, check that subject matches B AND the previous sibling of subject matches A.
  - Need access to `siblings: &[&Element]` and `index_in_siblings: usize`.
- Add `GeneralSibling` arm:
  - Same but any preceding sibling matches A.
- Update `match_full()` signature to accept `siblings` and `sibling_index` parameters.

#### 4. `crates/uwebr-render/src/layout.rs`
- In `build_tree()`: for elements with `::before` / `::after` CSS rules:
  1. Create synthetic `Element { node_type: NodeType::Text(content), ... }`.
  2. Insert as first/last child of the parent element.
  3. The synthetic element gets a generated `node_id`.
- In `collect_positioned_nodes()`: pass sibling list to style matching.

#### 5. `crates/uwebr-css/src/codegen.rs`
- `extract_paint()` needs to handle the `content` property for pseudo-elements:
  - `"content"` → store as `PaintProps.content: Option<String>`.
- Add `PaintProps.content: Option<String>` field.

### Tests (10+ tests)
- `test_selector_adjacent_sibling`: `h1 + p` matches p immediately after h1.
- `test_selector_general_sibling`: `h1 ~ p` matches any p after h1.
- `test_pseudo_element_before`: `::before { content: "»" }` creates synthetic text node.
- `test_pseudo_element_after`: `::after { content: "..." }`.
- `test_pseudo_element_with_content_prop`: content property on ::before.
- `test_adjacent_no_match_skips`: `h1 + p` doesn't match when span is between.
- `test_general_sibling_matches_all`: `h1 ~ p` matches all following p siblings.
- `test_pseudo_element_inherits_styles`: ::before inherits font/color from parent.
- `test_sibling_combinator_specificity`: adjacent + pseudo-element specificity calculation.

---

## Faz 4: `display: block` / `inline` / `inline-block`

### Goal
Support `display: block`, `display: inline`, `display: inline-block` layout modes.

### Architecture Insight
- Currently `element_to_style()` in layout.rs maps all block-level tags to `Flex + Column`
  and inline tags to `Flex + Row` as a default.
- Taffy does NOT have native block/inline layout. We simulate it via Flex.
- `display: block` → `FlexDirection::Column` + `Display::Flex` (current behavior for div/h1-h6).
- `display: inline` → `FlexDirection::Row` + `Display::Flex` + `FlexWrap::Wrap`.
- `display: inline-block` → `Display::Flex` with no wrap + inline-like sizing.
- `display: none` already works (`Display::None`).

### Files to Modify

#### 1. `crates/uwebr-css/src/codegen.rs`
- In `apply_property()`: handle `"display"` values:
  - `"block"` → set `Display::Flex` + mask `flex_direction`.
  - `"inline"` → set `Display::Flex` + `FlexDirection::Row` + `FlexWrap::Wrap`.
  - `"inline-block"` → set `Display::Flex`.
  - `"flex"` → already handled.
  - `"grid"` → already handled.
  - `"none"` → already handled.

#### 2. `crates/uwebr-render/src/layout.rs`
- In `element_to_style()`: when `mask.display` is false (no explicit display rule):
  - Keep current tag-default behavior (block→flex-column, inline→flex-row).
- When `mask.display` is true (explicit display rule):
  - `"block"` → `Display::Flex` + `FlexDirection::Column` + width 100%.
  - `"inline"` → `Display::Flex` + `FlexDirection::Row` + `FlexWrap::Wrap`.
  - `"inline-block"` → `Display::Flex` + intrinsic sizing.
- Add `width: 100%` for block elements when width is not set (standard block behavior).

### Tests (6+ tests)
- `test_display_block_full_width`: block element takes 100% width.
- `test_display_inline_flow`: inline elements flow in a row.
- `test_display_inline_block`: inline-block has intrinsic width but flows inline.
- `test_display_none`: already works, verify no regression.
- `test_display_override_tag_default`: `<span style="display: block">` → block layout.
- `test_display_flex_still_works`: existing flex layout unaffected.

---

## Implementation Order

| Order | Faz | Effort | Risk |
|-------|-----|--------|------|
| 1 | CSS Variables + calc() | High | Medium |
| 2 | Media Query Evaluation | Low | Low |
| 3 | Pseudo-elements + Sibling | High | Medium |
| 4 | Block/inline Layout | Medium | Low |

### Dependency Graph
```
Faz 1 (variables/calc) ──────────────────┐
Faz 2 (media queries) ──────────────────┤ (independent, parallel OK)
Faz 3 (pseudo-elements + sibling) ──────┤ (independent, parallel OK)
Faz 4 (block/inline) ───────────────────┘
```

All 4 faz are independent and can be implemented in any order. The order above
is chosen for: (a) most requested features first, (b) foundational → specific.
