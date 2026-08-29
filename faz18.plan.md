# FAZ 18 — Entegrasyon + Benchmark <500ms Hedefi

**Hedef:** FAZ 16-17'deki dinamik library ve hot-swap mekanizmasını `uwebr dev` komutuna bağlamak ve hot reload süresini <500ms'e düşürmek.

**Ön koşul:** FAZ 16 ve FAZ 17 tamamlandı.

**Sebep:** FAZ 16 shared library produce ediyor, FAZ 17 runtime load/swap yapıyor. Ama ikisi henüz `dev_server`'a bağlı değil. Bu faz her şeyi birleştirir.

**Tahmini süre:** ~3-4 saat
**Test hedefi:** +8-10 test + benchmark

---

## Adım 1 — `dev_server`'ı yeniden yapılandır

Mevcut `commands.rs:dev_server` fonksiyonunu iki moda böl:

```rust
pub enum ReloadMode {
    /// Mevcut mod: cargo build + process restart (~7s)
    FullRestart,
    /// Yeni mod: shared library compile + in-process swap (<500ms)
    HotSwap,
}

pub fn dev_server(path: &str) -> Result<()> {
    dev_server_with_mode(path, ReloadMode::HotSwap)
}

pub fn dev_server_with_mode(path: &str, mode: ReloadMode) -> Result<()> {
    match mode {
        ReloadMode::FullRestart => dev_server_full_restart(path),
        ReloadMode::HotSwap => dev_server_hot_swap(path),
    }
}
```

---

## Adım 2 — `dev_server_hot_swap` fonksiyonu

Yeni hot-swap dev server'ın akışı:

```
Dosya değişimi algılama (notify)
    ↓
Değişiklik sınıflandırma (classify_changes)
    ↓
┌─ CSS-only ──→ StyleBook reparse (mevcut, ~32µs) → hot-swap CSS
├─ Full ──────→ compile_shared_library → hot-swap library
└─ None ──────→ atla
```

```rust
fn dev_server_hot_swap(path: &str) -> Result<()> {
    let root = PathBuf::from(path);

    // 1. İlk derleme — full compile (ilk seferde cargo build gerekli)
    let mut cache = BuildCache::new(root.clone());
    cache.build_all()?;
    transpile_all(&root)?;
    cargo_build(&root)?;

    // 2. İlk library'yi yükle
    let dynlib_dir = root.join("target/dynlib");
    let component_name = root_component_name(&root)?;
    let mut swap_manager = HotSwapManager::new(dynlib_dir.clone(), component_name.clone());
    swap_manager.load_initial()?;

    // 3. İlk CSS'i yükle
    if let Some(css) = swap_manager.css() {
        let stylebook = StyleBook::parse(&css)?;
        // StyleBook'u pencereye uygula
    }

    // 4. App'i başlat (ilk yükleme ile)
    let mut app = App::new(&component_name);
    // ... window setup ...

    // 5. File watcher başlat
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(...)?;
    watcher.watch(root.join("src").as_path(), RecursiveMode::Recursive)?;

    // 6. Hot-swap event loop
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => {
                let changed = debounce(&rx, event);
                let kind = classify_changes(&changed);

                match kind {
                    ChangeKind::CssOnly => {
                        // CSS hot-swap: sadece StyleBook yeniden parse et
                        let start = Instant::now();
                        if let Some(new_css) = /* CSS dosyasını oku */ {
                            if let Ok(sb) = StyleBook::parse(&new_css) {
                                // StyleBook'u uygula
                                println!("  CSS reloaded in {:?}", start.elapsed());
                            }
                        }
                    }
                    ChangeKind::Full => {
                        let start = Instant::now();

                        // 1. Transpile
                        let uwebr_files: Vec<_> = changed.iter()
                            .filter(|p| p.extension() == Some("uwebr"))
                            .cloned()
                            .collect();
                        transpile_incremental(&root, &uwebr_files)?;

                        // 2. Shared library compile
                        let lib_path = next_version_path(&dynlib_dir, &component_name, &mut version);
                        let content = fs::read_to_string(changed.first().unwrap())?;
                        compile_shared_library(&content, &component_name, &CompileOptions {
                            root: root.clone(),
                            target_dir: dynlib_dir.clone(),
                            profile: CompileProfile::Debug,
                        })?;

                        // 3. Hot-swap
                        match swap_manager.try_swap(&lib_path) {
                            Ok(result) => {
                                println!("  hot-reloaded in {:?} (css_changed={})",
                                    start.elapsed(), result.css_changed);
                                // Yeni CSS varsa StyleBook güncelle
                                if result.css_changed {
                                    if let Some(css) = swap_manager.css() {
                                        if let Ok(sb) = StyleBook::parse(&css) {
                                            // uygula
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("  swap failed: {e} — keeping current version");
                            }
                        }
                    }
                    ChangeKind::None => {}
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                // Redraw check
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}
```

---

## Adım 3 — App'e hot-swap desteği ekleme

`uwebr-app/src/app.rs`'e method ekle:

```rust
impl App {
    /// Component'i runtime'da değiştir (hot-swap için)
    pub fn swap_component(&mut self, new_component: Box<dyn Component>) {
        self.component = Some(new_component);
        // Pencereyi yeniden çiz
        for window in self.windows.values_mut() {
            window.render();
        }
    }

    /// StyleBook'u runtime'da değiştir (CSS hot-swap için)
    pub fn swap_stylebook(&mut self, new_stylebook: StyleBook) {
        self.stylebook = Some(new_stylebook);
        // Pencereyi yeniden çiz
        for window in self.windows.values_mut() {
            window.render();
        }
    }
}
```

**Not:** Bu FAZ'da `swap_component` sadece `FnComponent` ile çalışacak. `FnComponent`, `LoadedLibrary.render()` çağrısını sarmalayan bir closure kullanacak:

```rust
let swap_mgr = Arc::new(Mutex::new(swap_manager));
let component = FnComponent::new({
    let mgr = swap_mgr.clone();
    move || {
        let mgr = mgr.lock().unwrap();
        let ptr = mgr.render();
        unsafe { *Box::from_raw(ptr) }
    }
});
```

---

## Adım 4 — CLI komutu güncelleme

```bash
# Eski mod (full restart)
uwebr dev --mode restart

# Yeni mod (hot-swap, varsayılan)
uwebr dev
uwebr dev --mode hot-swap

# Benchmark
uwebr bench-reload  # 10 kez hot-swap yap ve süreleri ölç
```

`clap` güncellemesi:

```rust
#[derive(Parser)]
enum Commands {
    Dev {
        #[arg(long, default_value = "hot-swap")]
        mode: String,
    },
    BenchReload,
}
```

---

## Adım 5 — Benchmark: `uwebr bench-reload`

Gerçek hot reload süresini ölç:

```rust
pub fn bench_reload_command(path: &str) -> Result<()> {
    let root = PathBuf::from(path);
    let dynlib_dir = root.join("target/dynlib");
    let component_name = root_component_name(&root)?;

    // İlk compile
    let mut cache = BuildCache::new(root.clone());
    cache.build_all()?;
    transpile_all(&root)?;

    let mut version = 0;
    let mut times = vec![];

    for i in 0..10 {
        // .uwebr dosyasını hafifçe değiştir (i sayısını ekle)
        let content = fs::read_to_string(root.join("src/App.uwebr"))?;
        let modified = content.replace(
            &format!("Count: {{count}}"),
            &format!("Count: {count} (v{i})"),
        );
        fs::write(root.join("src/App.uwebr"), &modified)?;

        let start = Instant::now();

        // Transpile
        transpile_incremental(&root, &[root.join("src/App.uwebr")])?;

        // Compile shared library
        let lib_path = next_version_path(&dynlib_dir, &component_name, &mut version);
        compile_shared_library(&modified, &component_name, &CompileOptions {
            root: root.clone(),
            target_dir: dynlib_dir.clone(),
            profile: CompileProfile::Debug,
        })?;

        // Hot-swap (eğer running app varsa)
        // Simülasyon: sadece load + render
        let lib = LoadedLibrary::load(&lib_path)?;
        let ptr = lib.render();
        unsafe { Box::from_raw(ptr); }

        let elapsed = start.elapsed();
        times.push(elapsed);
        println!("  Reload #{i}: {elapsed:?}");
    }

    let avg: Duration = times.iter().sum::<Duration>() / times.len() as u32;
    let min = times.iter().min().unwrap();
    let max = times.iter().max().unwrap();
    println!("\n--- Results ---");
    println!("  Average: {avg:?}");
    println!("  Min:     {min:?}");
    println!("  Max:     {max:?}");
    println!("  Target:  <500ms");
    if avg < Duration::from_millis(500) {
        println!("  Status:  PASS");
    } else {
        println!("  Status:  FAIL (optimize needed)");
    }

    Ok(())
}
```

---

## Adım 6 — Zamanlama tahminleri

| Adım | Beklenen süre |
|---|---|
| classify_changes | ~80 ns |
| transpile_incremental | ~50-400 µs |
| compile_shared_library | ~1-3s (cargo build --lib) |
| LoadedLibrary::load | ~5-20 ms |
| render() çağrısı | ~1-5 ms |
| **Toplam** | **~1.5-3.5s** |

**Hedef <500ms henüz ulaşılamıyor** — asıl darboğaz `compile_shared_library` (yine `cargo build --lib`).

**Optimizasyon yolları (gelecek FAZ):**
1. **sccache** kullan → compile süresini ~%50 düşürür
2. **Incremental compilation** → sadece değişen dosyayı derle
3. **Pre-compiled stub** → Component stub'unu önceden derle, sadece body'yi değiştir
4. **Interpreter modu** → Rust derleme yerine AST yorumlama (çok karmaşık, uzun vadeli)

Bu FAZ'da hedef: **<3s** (mevcut ~7s'den %50+ iyileşme). Gerçek <500ms için optimizasyon FAZ'ları gerekecek.

---

## Adım 7 — Fallback mekanizması

Hot-swap başarısız olursa otomatik fallback:

```rust
fn reload_with_fallback(
    root: &Path,
    swap_manager: &mut HotSwapManager,
    changed: &[PathBuf],
) -> Result<()> {
    // 1. Dene: shared library compile + swap
    match try_hot_swap(root, swap_manager, changed) {
        Ok(result) => {
            println!("  hot-reloaded in {:?}", result.render_time_ms);
            return Ok(());
        }
        Err(e) => {
            eprintln!("  hot-swap failed: {e}");
            eprintln!("  falling back to full restart...");
        }
    }

    // 2. Fallback: cargo build + process restart
    cargo_build(root)?;
    // Process restart mevcut akışı
    Ok(())
}
```

---

## Adım 8 — Testler

### 8.1 dev_server hot-swap modu testi
- `dev_server_with_mode(path, ReloadMode::HotSwap)` başlatılabilmeli
- İlk cargo build başarılı olmalı

### 8.2 CSS-only hot-swap testi
- CSS dosyası değiştiğinde transpile atlanmalı
- StyleBook güncellenmeli
- `cargo build` çalıştırılmamalı

### 8.3 Full hot-swap testi
- .uwebr dosyası değiştiğinde shared library compile edilmeli
- Hot-swap başarılı olmalı
- Eski library unload edilmeli

### 8.4 Fallback testi
- Geçersiz .uwebr content → hot-swap başarısız → fallback triggers
- Fallback sonrası uygulama hâlâ çalışmalı

### 8.5 bench-reload testi
- 10 reload süresi ölçülmeli
- Ortalama <3s olmalı (hedef)

### 8.6 state after swap testi
- Hot-swap sonrası component render edilebilmeli

### 8.7 CSS unchanged skip testi
- CSS değişmediyse StyleBook parse edilmemeli

### 8.8 concurrent file changes testi
- 3 dosya aynı anda değişmeli → debounce + batch reload

### 8.9 version monotonic testi
- Her hot-swap versiyon numarası artmalı

### 8.10 binary size testi
- Shared library boyutu makul olmalı (<10MB debug'da)

---

## Doğrulama

1. `cargo test --workspace` → tüm testler yeşil
2. `cargo clippy --workspace` → 0 warning
3. `cargo fmt --check` → temiz
4. **Manuel test zinciri:**
   ```bash
   # Terminal 1: scaffold projesini başlat
   cd scaffold && cargo run -p uwebr-cli -- dev --mode hot-swap

   # Terminal 2: .uwebr dosyasını değiştir
   echo "modified" >> scaffold/src/App.uwebr

   # Terminal 1'de çıkış:
   #   [reload] 1 file(s): src/App.uwebr
   #   compiled shared library in 1.8s
   #   hot-reloaded in 25ms
   ```
5. **Benchmark:**
   ```bash
   cargo run -p uwebr-cli -- bench-reload --path scaffold/
   ```
   Çıktı: `Average: <3s` olmalı

---

## FAZ 16-17-18 Toplam Özeti

| FAZ | İçerik | Test | Süre |
|---|---|---|---|
| FAZ 16 | `uwebr-dynlib` crate, shared library compile | +6-8 | ~2-3h |
| FAZ 17 | Runtime loader, hot-swap manager, error recovery | +10-12 | ~3-4h |
| FAZ 18 | `dev_server` entegrasyonu, benchmark, fallback | +8-10 | ~3-4h |
| **Toplam** | | **+24-30** | **~8-11h** |

**Sonuç:** Hot reload ~7s → ~2-3s (compile hâlâ darboğaz). Gerçek <500ms için ileri optimizasyon FAZ'ları gerekecek.
