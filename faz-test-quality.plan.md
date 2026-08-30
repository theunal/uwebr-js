# FAZ: Test Kalitesi + Yeni Alanlar

## Hedef
- ~68 duplicate/trivial test sil
- ~140 farklı ve anlamlı test ekle
- Sonuç: ~1675 test, hepsi anlamlı

## Aşama 1: Temizlik (~68 test sil)

### dynlib/src/compiler.rs — ~25 test sil
- `dynlib_to_snake_*` (6) — transpiler.rs ile aynı
- `dynlib_extract_tag_*` (5) — transpiler ile aynı
- `dynlib_extract_html_*` (5) — transpiler ile aynı
- `dynlib_extract_css_*` (5) — transpiler ile aynı
- `dynlib_extract_event_handlers_*` (4) — transpiler ile aynı

### dynlib/src/swap.rs — ~12 test sil
- Trivial getter: `dynlib_hot_swap_manager_library_dir`, `_component_name`, `_non_existent_dir`, `dynlib_version_starts_at_zero`, `dynlib_hot_swap_manager_initial_version_zero_after_new`
- Near-duplicate: `dynlib_next_version_path_sequential_increments`, `_preserves_dir`, `dynlib_versioned_filename_long_name`, `_version_zero`
- Trivial display: `dynlib_swap_error_*_display_contains_*` (3)

### render/src/scene.rs — ~11 test sil
- derive/Default/constructor tests

### render/src/renderer.rs — ~10 test sil
- Getter/setter/flag tests

### render/src/text.rs — ~4 test sil
- `>= 0.0` trivial tests

### render/src/metrics.rs — ~6 test sil
- Near-duplicate range tests

## Aşama 2: Yeni Testler (~140 test)

### uwebr-core (~65 test)
Hata Yolları: re-entrant effect, memo convergence, type mismatch, router unknown path, lifecycle noop
Edge Cases: prop NaN/inf, diff out-of-bounds, replace root, insert clamp
Thread Safety: timer cross-thread, concurrent tick, cancel racing
Memory: signal clone survive drop, setter clone, ID uniqueness
Integration: state→effect, event→hover→stylebook
Stress: 10000 node diff, 1000 signal batch, 500 memo chain

### uwebr-render (~40 test)
Hata Yolları: invalid CSS parse, empty angle brackets, unclosed quotes
Edge Cases: invalid hex, transparent alpha, px variants, empty class
Integration: reparse, StyleMask union, 3-level inheritance, inline override CSS
Stress: 500 rules, 1000 nodes, rapid relayout

### uwebr-css (~15 test)
Edge Cases: invalid hex chars, no-hash hex, shorthand expansion, calc/var/clamp

### uwebr-html (~10 test)
Edge Cases: entities, 50-level nesting, script/style content preservation

### uwebr-js (~10 test)
New patterns: arrow functions, template literals, destructuring, optional chaining

## Aşama 3: Doğrulama
1. cargo fmt
2. cargo clippy --workspace
3. cargo test --workspace
