# Form Inputs Implementation Plan

## Goal

Add interactive form components (TextInput, Button, Checkbox, Radio) to the uwebr framework, enabling users to build data-entry UIs.

## Architecture

### Key Design Decisions

1. **Immediate-mode rendering**: The element tree is rebuilt every frame. Input state (value, cursor, checked) lives in signals, not on elements.
2. **Focus system**: Click sets `focused` node_id. Keyboard events route to the focused element.
3. **Event routing**: Extend the pipeline's hit-target extraction to support `on:keydown` and `on:focus` props, analogous to `on:click`.
4. **Native rendering**: Inputs are rendered by vello (not native OS widgets). The framework owns all rendering.

### Component Mapping

| HTML Tag | Component | Key State | Events |
|----------|-----------|-----------|--------|
| `<input type="text">` | TextInput | value: String, cursor: usize | on:input, on:focus, on:blur |
| `<button>` | Button | (none) | on:click, on:keydown(Enter/Space) |
| `<input type="checkbox">` | Checkbox | checked: bool | on:change |
| `<input type="radio">` | Radio | checked: bool | on:change |

---

## FAZ 1: Focus System + Keyboard Event Routing

### Goal
Make focus work end-to-end: click an element → it gains focus → keyboard events route to it → focus CSS pseudo-class applies.

### Files to Modify

#### 1. `crates/uwebr-core/src/events.rs`
- Add `on:keydown` and `on:focus` prop extraction helpers (like `click_action()` in pipeline.rs)

#### 2. `crates/uwebr-app/src/pipeline.rs`
- Add `KeyTarget` struct: `{ action: String, bounds: LayoutInfo, depth: usize }` (parallel to `HitTarget`)
- Add `FocusTarget` struct: `{ node_id: usize, bounds: LayoutInfo }`
- Add `key_targets: Vec<KeyTarget>` field to `RenderPipeline`
- Add `focus_targets: Vec<FocusTarget>` field to `RenderPipeline`
- Add `key_action()` free function (parallel to `click_action()`) that extracts `on:keydown` prop
- In `build_render_scene()`: populate `key_targets` by calling `key_action()` on each positioned node
- Add `key_hit_test(x, y) -> Option<&str>` method (find innermost key target at point)
- Add `focus_hit_test(x, y) -> Option<usize>` method (find innermost focusable node at point)
- Add `key_targets()` accessor

#### 3. `crates/uwebr-app/src/app.rs`
- In `MouseInput` handler: after `handle_click()`, also call `state.handle_focus()` to set/clear focus
- Add `handle_focus()` method to `WindowState`:
  - If clicked element has `on:focus` or is an input-like element → `set_focused(Some(node_id))`
  - If clicked on empty space → `set_focused(None)`
  - Re-render with `:focus` CSS applied
- In `KeyboardInput` handler:
  - If `any_focused()` → extract key name → look up focused element's `on:keydown` action → dispatch
  - If no focused element → current behavior (generic AppEvent::KeyPress)

#### 4. `crates/uwebr-render/src/stylebook.rs`
- Ensure `:focus` pseudo-class already works (check `test_pseudo_class_focus_matches` — it exists at line in stylebook tests)
- If not wired, add focus state check in `selector_matches()`

### Tests
- Focus on click, blur on empty space click
- Keyboard event dispatched to focused element's `on:keydown` handler
- `:focus` CSS pseudo-class applies when element focused
- Multiple focusable elements: only one focused at a time

---

## FAZ 2: TextInput Component

### Goal
A text input field that displays typed text, shows a blinking caret, supports cursor movement, text selection, and clipboard.

### State Model

Each TextInput manages its own state via signals:

```rust
let (value, set_value) = use_state("input_value", String::new());
let (cursor, set_cursor) = use_state("input_cursor", 0usize);
let (sel_start, set_sel_start) = use_state("input_sel_start", 0usize);
let (sel_end, set_sel_end) = use_state("input_sel_end", 0usize);
```

### Files to Modify

#### 1. `crates/uwebr-render/src/text.rs`
- Add `measure_advance_before(content, font_size, font_family, char_index) -> f32`
  - Walk parley glyph runs, sum advances up to `char_index`
  - Used for caret x-position and selection highlight positioning
- Add `char_index_at_x(content, font_size, font_family, x: f32) -> usize`
  - Reverse of above: find which character the user clicked at
  - Walk glyph runs, find character whose advance range contains `x`

#### 2. `crates/uwebr-render/src/scene_builder.rs`
- Add `draw_caret(x, y, height, color)` method — thin filled rect (1-2px wide)
- Add `draw_selection_rect(x, y, width, height, color)` — semi-transparent highlight behind selected text
- Modify `draw_text()` or add `draw_input_text()` variant that also draws caret + selection

#### 3. `crates/uwebr-render/src/layout.rs`
- Add `InputContext` variant to `NodeContext`:
  ```rust
  NodeContext::Input {
      value: String,
      font_size: f32,
      font_family: Option<String>,
      cursor: usize,
      sel_start: usize,
      sel_end: usize,
      focused: bool,
  }
  ```
- In `measure_callback`: handle `InputContext` same as `Text` for size measurement
- This tells taffy how much space the input needs

#### 4. `crates/uwebr-app/src/pipeline.rs`
- Detect `<input>` elements during `build_render_scene()` and record them in a new `input_nodes: HashMap<usize, InputNodeInfo>` map
- `InputNodeInfo { value: String, cursor: usize, sel_start, sel_end, font_size, font_family }`
- Expose `input_nodes()` accessor

#### 5. `crates/uwebr-app/src/app.rs`
- On `MouseInput` on a focused `<input>`:
  - Use `char_index_at_x()` to position caret where user clicked
- On `KeyboardInput` on a focused `<input>`:
  - Printable characters → insert at cursor, advance cursor
  - Backspace → delete char before cursor
  - Delete → delete char after cursor
  - Left/Right arrows → move cursor
  - Shift+Left/Right → extend selection
  - Ctrl+A → select all
  - Ctrl+C → copy to clipboard
  - Ctrl+V → paste from clipboard
  - Ctrl+X → cut selection
  - Home/End → move to start/end
- After mutation → mark UI dirty → re-render
- Render phase: scene_builder draws the input text + caret + selection highlight

#### 6. `crates/uwebr-render/src/scene_builder.rs` (rendering)
- In `draw_node()`: detect `<input>` elements, render:
  1. Background + border (already works via CSS)
  2. Text value (truncated to visible width if needed)
  3. Caret (blinking via animation timer — toggle every 500ms)
  4. Selection highlight (blue rect behind selected chars)

### Blinking Caret
- Use a simple timer: toggle `caret_visible` every 500ms when input is focused
- `caret_visible` stored in `WindowState` or `RenderPipeline`
- Reset blink on any keypress (standard UX)

### Tests
- Input renders with value text
- Typing inserts characters at cursor position
- Backspace deletes character before cursor
- Arrow keys move caret
- Shift+Arrow selects text
- Ctrl+A selects all
- Selection highlight renders
- Caret blinks

---

## FAZ 3: Button Component

### Goal
A styled button that responds to click and keyboard (Enter/Space), with hover/active/focus/disabled states.

### Files to Modify

#### 1. `crates/uwebr-render/src/layout.rs`
- No structural changes needed — `<button>` is just a `<div>` with special CSS

#### 2. `crates/uwebr-app/src/app.rs`
- In `KeyboardInput` handler:
  - If focused element is a `<button>` and key is Enter or Space → dispatch its `on:click` action
  - This makes buttons keyboard-accessible

#### 3. CSS defaults (in `element_to_style()` or tag defaults)
- `<button>` gets default styling:
  - `cursor: pointer`
  - `padding: 8px 16px`
  - `background: #e0e0e0`
  - `border: 1px solid #999`
  - `border-radius: 4px`
  - `text-align: center`

#### 4. `:active` pseudo-class (optional, nice-to-have)
- Track "active" state (mouse down on element, before mouse up)
- Add `set_active()`/`is_active()` to `ElementStateStore`
- CSS `:active` rules apply during mousedown

### Tests
- Button renders with text
- Click dispatches on:click action
- Enter/Space on focused button dispatches on:click
- Disabled button does not respond to click
- Button has cursor: pointer by default

---

## FAZ 4: Checkbox + Radio Components

### Goal
Toggle controls with visual feedback (checkmark/radio dot).

### Files to Modify

#### 1. `crates/uwebr-render/src/scene_builder.rs`
- Add `draw_checkmark(x, y, size, color)` — draws a checkmark (two line segments or a path)
- Add `draw_radio_dot(x, y, size, color)` — draws a filled circle

#### 2. `crates/uwebr-app/src/app.rs`
- On click of `<input type="checkbox">`:
  - Toggle the `checked` state signal
  - Dispatch `on:change` action if present
- On click of `<input type="radio">`:
  - Uncheck other radios in same `name` group
  - Check this radio
  - Dispatch `on:change` action if present
- On Space key on focused checkbox/radio → toggle (same as click)

#### 3. CSS defaults
- Checkbox: small square (16x16), border, border-radius: 2px
- Radio: small circle (16x16), border, border-radius: 50%
- Checked state: filled background + checkmark/dot

### Tests
- Checkbox toggles on click
- Checkbox toggles on Space key
- Radio checks on click, unchecks siblings with same name
- Visual rendering: checkmark appears when checked
- `:checked` CSS pseudo-class applies

---

## Implementation Order

1. **FAZ 1** (Focus + Keyboard routing) — foundation, no visible components yet
2. **FAZ 2** (TextInput) — most complex, builds on FAZ 1
3. **FAZ 3** (Button) — simple, extends FAZ 1
4. **FAZ 4** (Checkbox + Radio) — extends FAZ 1 + rendering

## Estimated Scope

| FAZ | New/Modified Files | Approx. Lines | Tests |
|-----|-------------------|---------------|-------|
| 1 | pipeline.rs, app.rs, events.rs | ~200 | 8-10 |
| 2 | text.rs, scene_builder.rs, layout.rs, pipeline.rs, app.rs | ~400 | 15-20 |
| 3 | app.rs, layout.rs | ~80 | 5-8 |
| 4 | scene_builder.rs, app.rs | ~150 | 8-10 |
| **Total** | **8 files** | **~830** | **36-48** |
