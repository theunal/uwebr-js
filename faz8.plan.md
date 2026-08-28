# FAZ 8: Son Kilometre — Ekranda Görünen Uygulama ✅ TAMAMLANDI

> Tespit: 2026-08-28 · HEAD `6bc3ea6`
> Uygulama: 2026-08-28 · `cargo test --workspace` → **444 test geçti**

## Neden Bu Faz Vardı

Projeden istenen özgün gereksinim: `.uwebr` dosyalarını (HTML + `<style>` + `<script>`) **hot reload** ile parse edip **Rust koduna** çevirmek ve **desktop** ortamında render etmek.

PLAN.md FAZ 0–7'yi tamamlanmış işaretliyordu. Kod okunarak yapılan doğrulama, mimarinin ve crate sınırlarının doğru olduğunu, ancak gereksinimin üç ayağından yalnız birinin tam olduğunu gösterdi:

| Ayak | Faz öncesi | Faz sonrası |
|------|------------|-------------|
| parse → Rust codegen | ✅ Gerçek ve çalışıyor | ✅ + script state, interpolasyon, event bağlama |
| desktop render | ⚠️ Pencere + GPU gerçek, metin ve CSS boyası ekrana çıkmıyor | ✅ Metin glyph olarak, CSS boyası ekranda |
| hot reload | ❌ Yeniden derliyor, çalışan uygulamayı güncellemiyor | ✅ Süreç kill/respawn, ~6.9 s |

Faz öncesi pratik sonuç: `uwebr init demo && cd demo && uwebr dev` yapıldığında **`cargo build` bile başarısız oluyordu** (modül kapsamında `let`, eksik `src/generated/`), pencere açılsa da `<h1>Hello from uwebr!</h1>` metni görünmüyordu.

Faz sonrası: `uwebr init demo` → `cargo build` geçiyor, pencere açılıyor, metin `#e0e0e0` renginde `#1a1a2e` zemin üzerinde görünüyor, dosya değişimi uygulamayı yeniden başlatıyor.

---

## Yapılan İşler

### M1 — Metin render (parley → vello) ✅

**Sorun üç katmanlıydı:**

1. `scene_builder.rs`'teki `RenderNodeKind::Text` kolu metin içeriğini atıp sabit 100 px genişlikte placeholder dikdörtgen çiziyordu.
2. `layout.rs` `TaffyTree<()>` kullanıyordu; text node'ları `new_leaf` ile ölçüsüz ekleniyor → column flex içinde 0 yükseklik.
3. `pipeline.rs::positioned_to_render_node` başındaki `if layout.width <= 0.0 || layout.height <= 0.0 { return None; }` 0 boyutlu text node'unu sahneden düşürüyordu.

Yani "metin yerine renkli kutu görürsün" bile iyimserdi; hiçbir şey çıkmıyordu.

**Yapılan:**

- `layout.rs`: `TaffyTree<NodeContext>` — text node'lar içeriğini/font'unu taşıyor. `compute_layout` → `compute_layout_with_measure`; measure closure `TextRenderer::measure`'a düşüyor.
- `scene_builder.rs`: placeholder `Rect` kaldırıldı; parley `Layout` → `line.items()` → `GlyphRun` → `scene.draw_glyphs(font).font_size(..).brush(color).draw(glyphs)`.
- `pipeline.rs`: erken dönüş metin için içerik kontrolüne çevrildi (`content.trim().is_empty()`), kutu boyutu kontrolü yalnız Element/Component için kaldı.
- `text.rs`: `estimate_text_size` eklendi — parley sistem fontu bulamazsa (headless CI, minimal image) 0 boyut döner ve node yine düşerdi.
- Ek olarak `collect_positioned_nodes` mutlak koordinat üretiyor: taffy ebeveyne göre konum verir, sahne tek düz uzayda çizer.

**Doğrulama:** `cargo run -p uwebr-render --example glyph_probe` → `glyphs=17 glyph_runs=1`, ölçüm `188.06 x 27.60`.

### M2 — CSS boyası ekrana çıkıyor ✅

**Sorun:** `stylebook.rs` çıktısı `Vec<(String, taffy::Style)>` idi. Taffy yalnız yerleşim bilir; `background-color`, `color`, `font-size`, `font-family` bu sınırda düşüyordu. `color.rs::css_color_to_peniko` ölü koddu. `pipeline.rs::extract_text_style` `font_size`'ı sadece `PropValue::Number`'dan okuyordu; transpiler her literal attribute'u `String` ürettiği için font boyutu daima 16.0 default'unda kalıyordu.

**Yapılan:**

- `uwebr-css/codegen.rs`: `PaintProps` (background/color/font-size/font-family/border-color/border-width/border-radius/opacity) + `extract_paint()` + `StyleEntry` + `convert_to_style_entries()`. Eski `convert_to_taffy_styles()` API'si korundu.
- `uwebr-render/paint.rs` (yeni): `ResolvedPaint` — kalıtılan < CSS kuralı < inline prop önceliği. `color`/`font_size`/`font_family` çocuklara kalıtılır; `background`/`border`/`opacity` kalıtılmaz (CSS semantiği).
- `prop_to_f32` hem `Number` hem `String` kabul ediyor, `"28px"` gibi CSS'e benzer değerleri de tolere ediyor.
- `stylebook.rs::match_full()` → `MatchedStyle { style, mask, paint, matched }`.
- Metin node'ları kendi kuralına düşmez (selector'ı yoktur); boya ebeveynden kalıtımla gelir.
- `css_color_to_peniko` artık `ResolvedPaint::apply_css` içinden kullanılıyor — ölü kod değil.

**Yan bulgu:** `parse_length` `"2rem"`'i `em` kolunda yakalıyordu (`ends_with("em")` önce test ediliyordu), `"2r"` kalıp parse başarısız oluyordu. Sıra düzeltildi, iki regresyon testi eklendi.

### M3 — Cascade düzeltmesi ✅

**Sorun:** `merge_style` hedef `Style`'ın her alanını koşulsuz atıyordu. Yalnız `width` veren bir class kuralı, tag kuralından gelen `display`/`flex_direction`/`padding` değerlerini default'a resetliyordu.

**Yapılan:** `StyleMask` — her kuralın gerçekten set ettiği property'leri izleyen bit alanı. `apply_property` artık `(&mut Style, &mut StyleMask, name, value)` alıyor. `merge_style` yalnız mask'ta işaretli alanları yazıyor. Tag default'ları da per-property uygulanıyor: `h1 { font-size: 2rem }` elementle eşleşir ama layout property'si tanımlamaz, dolayısıyla block-level `flex-direction: column` default'u yine geçerli olmalı.

**Regresyon testi:** `test_three_level_cascade_tag_class_id` — tag + class + id aynı elemana, her seviye kendi property'sini katkılıyor, hiçbiri diğerini ezmiyor.

### M4 — Surface/storage texture ✅

**Tespitte "yalnız derleme düzeyinde incelendi" denmişti — çalıştırıldı ve panikledi:**

```
wgpu error: Validation Error
  In Device::create_bind_group
    Storage texture binding 5 expects format = Rgba8Unorm,
    but given a view with format = Bgra8UnormSrgb
```

**Yapılan (faz4.plan.md'deki yol):** `GpuContext` bir `Rgba8Unorm` + `STORAGE_BINDING | TEXTURE_BINDING` ara texture tutuyor, vello oraya çiziyor, `wgpu::util::TextureBlitter` ile surface'e blit ediliyor. `resize` ara texture'ı da yeniden yaratıyor. Surface formatı non-sRGB (`Rgba8Unorm`/`Bgra8Unorm`) seçiliyor — sRGB olsaydı transfer fonksiyonu iki kez uygulanırdı.

**Doğrulama:** `cargo run -p uwebr-app --example gpu_probe` — headless render + framebuffer readback:

```
render nodes: 3
encoded: 17 glyphs, 2 paths
distinct colours: 299
  #1a1a2e  77701 px
  #e0e0e0  947 px
OK: .app background #1a1a2e covers the surface
OK: 2299 px of foreground (text glyphs)
```

Ayrıca üretilen binary gerçek pencerede 7 s çalıştı, panik yok.

### M5 — Gerçek hot reload ✅

**Sorun:** `dev_server` `.uwebr` değişiminde transpile + `cargo build` yapıyordu ama **uygulama süreci hiç başlatılmıyordu**. "Dosya değişti → ekran güncellendi" akışı yoktu. `BuildCache` ölü koddu.

**Yapılan:**

- `AppProcess` — `std::process::Child` sarmalayıcısı; `spawn` / `is_alive` / `kill` (kill + wait, zombie bırakmaz).
- İlk build sonrası uygulama başlatılıyor; değişimde build → eski süreci `kill()` + `wait()` → yeniden spawn.
- Build hatasında child öldürülmüyor, çalışan sürüm ayakta kalıyor, hata terminale basılıyor.
- `BuildCache` `dev_server`'a bağlandı: parse tanılaması sağlıyor (`failing_files()`), hangi dosyanın parse edilemediği yazdırılıyor.
- Uygulama kapanırsa (crash ya da pencere kapatma) bir sonraki timeout'ta bildiriliyor.

**Windows'a özgü engel:** uygulama doğrudan `target/debug/app.exe`'den başlatıldığında Windows dosyayı kilitliyor ve sonraki `cargo build` link aşamasında başarısız oluyordu — bu, gerçek bir derleme hatasından ayırt edilemiyordu (ilk deneme "build failed — keeping the running app" yazıp sonsuza kadar takılı kaldı). Çözüm: uygulama build çıktısının bir kopyasından (`<name>-dev-run.exe`) çalıştırılıyor; kopya süreç ölünce siliniyor.

**Ölçüm:** dosya kaydından yeni sürecin ayağa kalkmasına **6.88 s**. Transpile adımı ~60 ms; kalanı `cargo build`.

### M6 — `<script>` ↔ template ↔ reaktivite ✅

**Sorun:** `uwebr-js` codegen'de `static`/`const` emisyon yolu yoktu. Top-level `let` `RsStmt::LetMut`'a çevrilip modül kapsamına yazılıyordu:

```rust
let mut count: i64 = 0;          // modül kapsamında `let` — Rust reddeder
fn increment() { count += 1; }   // `count` kapsamda değil
```

Yani taze scaffold'ın `cargo build` adımı başarısız oluyordu. Ayrıca script state'i ne `{count}` interpolasyonuna ne `uwebr_core` sinyallerine bağlıydı; signal/state değişimi hiçbir yerde repaint tetiklemiyordu.

**Yapılan:**

1. `uwebr-core/state.rs` (yeni): script binding'lerini isimle anahtarlanmış sinyallerde tutan store. `get(key, initial)` / `set(key, value)` / `clear()`. İsimle anahtarlı çünkü component fonksiyonu her render'da yeniden çağrılır ve aynı state'i görmesi gerekir.
2. `uwebr-js/script.rs` (yeni): `lower_script_state()` — her top-level binding'i getter/setter çiftine indirger, tüm gövdelerde referansları yeniden yazar (`count++` → `__set_state_count(__state_count() + 1)`). Yeni `transpile_script()` API'si `ScriptResult { code, warnings, states, functions }` döndürür.
3. `uwebr-core/signal.rs`: `RENDER_DIRTY` bayrağı + `mark_render_dirty` / `is_render_dirty` / `take_render_dirty`. Her `set`/`update` bayrağı kaldırır.
4. `uwebr-core/events.rs`: isimle anahtarlı action registry — `register_action` / `dispatch_action` / `has_action` / `clear_actions`. Handler kendini yeniden kaydedebilir (re-entrant borrow panigi yok).
5. `uwebr-cli/transpiler.rs`: `ScriptBindings` ile `{count}` → `__state_count()`, `on:click={increment}` → `PropValue::Closure("increment")` + component başında `register_action`. Tam identifier eşleşmesi (`counter`, `count` yüzünden yeniden yazılmaz).
6. `uwebr-app/pipeline.rs`: `HitTarget { action, bounds, depth }` + `hit_test(x, y)` — en derin hedef kazanır (DOM semantiği). Mutlak koordinat kullanır.
7. `uwebr-app/app.rs`: `CursorMoved` imleci izliyor, `MouseInput` sol tıkta `hit_test` → `dispatch_action`. `about_to_wait` artık timer **ve** `take_render_dirty()` kontrol ediyor.
8. `RawHtml` / `Comment` node'ları `is_emittable()` ile filtreleniyor — `vec![]` içinde çıplak `// ...` satırı üretmiyorlar artık.
9. Attribute ve metin değerleri Rust string literal'i için escape ediliyor (`title="say &quot;hi&quot;"` üretilen kodu bozuyordu).

**Doğrulama:** `crates/uwebr-app/tests/interaction_tests.rs` — 8 test: tıklama handler'ı çalıştırıyor, sayaç render'da güncelleniyor, UI dirty işaretleniyor, tekrarlı tıklama birikiyor, yeniden render state'i sıfırlamıyor.

### M7 — Scaffold derlenebilir ✅

**Sorun:** `init_project` `Cargo.toml`'a `uwebr-app = { git = "https://github.com/uwebr/uwebr" }` yazıyordu (repo muhtemelen yok), `src/main.rs`'e `mod generated;` yazıp `src/generated/` dizinini oluşturmuyordu, buna karşın "cd {name} / cargo run" öneriyordu.

**Yapılan:**

- `framework_dependencies()` — CLI'nin derlendiği uwebr checkout'u hâlâ varsa path dependency yazıyor; yoksa git'e düşüyor.
- `src/generated/` oluşturuluyor ve ilk transpile yapılıyor: `app.rs` + `mod.rs` + `main.rs`.
- Generated dosya adları snake_case (`MyPage.uwebr` → `my_page.rs`), `mod.rs`'teki modül adlarıyla eşleşiyor.
- `CSS_*` sabiti `pub` — `main.rs` `use generated::app::CSS_APP;` yapıyor, private const derlemede patlıyordu.
- Generated dosyalar `// @generated` başlığı + `#![allow(...)]` taşıyor; kullanıcının düzeltemeyeceği lint gürültüsü susturuluyor.
- Scaffold template'i artık `{count}` ve `on:click={increment}` içeriyor — reaktivite yolu ilk açılışta görünür.

**Yan bulgu:** `Count: {count}` gibi karışık satır içi içerik `div` ile sarılıyordu; block-level tag'lar column flex default'u aldığı için "Count:" ve sayı alt alta diziliyordu. `span` sarmalayıcıya çevrildi.

**Doğrulama:** `uwebr init` → `cargo build` başarılı → binary açılıyor.

---

## Bulgu Doğrulama Tablosu

Tespit belgesindeki her iddianın akıbeti:

| Bulgu | Durum | Not |
|-------|-------|-----|
| 1. KRİTİK — Metin hiç render edilmiyor | ✅ Doğruydu, çözüldü | Üç katman da onarıldı |
| 2. KRİTİK — Tüm CSS boya özellikleri kayboluyor | ✅ Doğruydu, çözüldü | `PaintProps` → `ResolvedPaint` |
| 3. KRİTİK — Hot reload çalışan uygulamayı güncellemiyor | ✅ Doğruydu, çözüldü | + Windows exe kilidi engeli |
| 4. KRİTİK — `<script>` geçersiz Rust üretiyor | ✅ Doğruydu, çözüldü | Derleyici hatası da gözlendi |
| 5. ORTA — Cascade bozuk | ✅ Doğruydu, çözüldü | `StyleMask` |
| 6. ORTA — Surface yolu faz4.plan.md'den sapıyor | ✅ Doğruydu, **çalıştırılınca panikledi** | Storage texture + blit |
| 7. ORTA — `uwebr init` derlenmeyen proje üretiyor | ✅ Doğruydu, çözüldü | + `pub const` bulgusu |
| 8. DÜŞÜK — Clipping yok | ⚠️ Kısmen | `push_clip_layer` bağlandı, ama CSS `overflow` boyaya taşınmıyor |
| 8. DÜŞÜK — `renderer.rs` ismi yanıltıcı | 📝 Dokümante edildi | İsim korundu (public API), faz4.plan.md açıklıyor |
| 8. DÜŞÜK — Component props callee'ye geçmiyor | ❌ Açık | Bilinen sınır; slot children düzeltildi |
| 8. DÜŞÜK — `RawHtml` / `Comment` bozuk kod üretiyor | ✅ Çözüldü | `is_emittable()` filtresi |

## Tespitte Yanlış Çıkan İddia Yok

Tespit belgesindeki üç "bayat madde" değerlendirmesi de doğruydu: `NodeType::Component` üretimi ve `prop_to_f32`'nin String kabulü gerçekten commit edilmemiş çalışma ağacında çözülmüştü.

Tek düzeltme: **PLAN.md'deki "283/283 test"** sayısı doğrulanmamıştı. Faz başlangıcında gerçek değer **288** çıktı; faz sonunda **444**.

---

## Doğrulama Sonuçları

| Kontrol | Komut | Sonuç |
|---------|-------|-------|
| Test suite | `cargo test --workspace` | 444 geçti, 0 başarısız |
| Format | `cargo fmt --all --check` | temiz |
| Lint | `cargo clippy --workspace --all-targets` | FAZ 8 dosyalarında uyarı yok (kalan 21 uyarı faz öncesi dosyalarda) |
| Metin + CSS ekranda | `cargo run -p uwebr-app --example gpu_probe` | 17 glyph, `#1a1a2e` zemin, 947 px `#e0e0e0` |
| Scaffold derleniyor | `uwebr init demo && cargo build` | başarılı |
| Uygulama açılıyor | üretilen binary | 7 s ayakta, panik yok |
| Hot reload | `uwebr dev` + dosya değişimi | yeni PID, 6.88 s |
| Etkileşim döngüsü | `cargo test -p uwebr-app --test interaction_tests` | 8 test |

### Test dağılımı

| Crate | Test |
|-------|-----:|
| uwebr-render | 93 |
| uwebr-core | 80 |
| uwebr-app | 79 |
| uwebr-cli | 72 |
| uwebr-css | 59 |
| uwebr-html | 31 |
| uwebr-js | 30 |
| uwebr-macro | 0 (proc-macro; testleri `uwebr-core/tests/macro_tests.rs`'te) |
| **Toplam** | **444** |

### Eklenen tanılama örnekleri

```bash
cargo run -p uwebr-render --example glyph_probe    # glyph üretimi + ölçüm
cargo run -p uwebr-render --example layout_probe   # font-size → text box yüksekliği
cargo run -p uwebr-app --example gpu_probe         # headless GPU render + framebuffer analizi
cargo run -p uwebr-cli --example scaffold_output   # scaffold'ın ürettiği Rust
```

---

## Revize Edilen Hedef

**Hot reload `< 500 ms` hedefi tutulamıyor.** Ölçülen 6.88 s ve neredeyse tamamı `cargo build`. Süreç yeniden başlatma modeliyle bu hedef ulaşılabilir değil; `< 500 ms` ancak in-process reload (dinamik kütüphane yükleme ya da yorumlanan bir katman) ile mümkün. ARCHITECTURE.md ölçülen değerle güncellendi.

## Açık Kalan Maddeler

Bunlar FAZ 8 kapsamı dışıydı ya da bilinçli olarak ertelendi:

- **Component props callee'ye geçirilmiyor.** `<Card title="x" />` prop'u `Element.props`'a yazılıyor ama `card_component()` argüman almıyor. Gerçek çözüm props struct'ı + `#[component]` makro entegrasyonu gerektirir.
- **`{@html expr}`** sahneye çıkmıyor (gerçek bir HTML alt-parser'ı gerekiyor); geçersiz Rust üretmiyor, sessizce düşüyor.
- **`RenderStyle::overflow_hidden`** CSS'ten doldurulmuyor. Sahne tarafı kırpmayı destekliyor, `pipeline.rs::paint_to_render_style` her zaman `false` yazıyor.
- **Pseudo-class / attribute selector'lar** parse ediliyor ama eşleşmede yok sayılıyor (`.btn:hover` → `.btn` gibi davranır).
- **`vw`/`vh`** yüzde olarak yaklaşılıyor: kökte viewport'a, iç içe elementlerde ebeveyne göre çözülür.
- **Gradient** CSS'ten gelmiyor. `Background::LinearGradient` ve `make_brush` desteği var, ancak `uwebr-css` `linear-gradient(...)`'ı `Keyword` olarak saklıyor.
- **Script shadowing:** `script.rs` rewriting'i identifier tabanlı; fonksiyon içindeki aynı adlı local, top-level binding ile karışabilir.
- **Ölçülmeyen metrikler:** FPS, bellek kullanımı, binary boyutu, cold start, 1000 node layout süresi.

---

*Oluşturma: 28 Ağustos 2026 · Tespit kaynağı: HEAD `6bc3ea6` · Uygulama: FAZ 8*
