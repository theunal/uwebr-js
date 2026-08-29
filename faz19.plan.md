# FAZ 19 — Hot Reload Optimizasyonu (sccache + temp reuse)

**Hedef:** Hot reload compile süresini ~3-5s → ~500ms-1s'e düşürmek.

**Ön koşul:** FAZ 16-17-18 tamamlandı.

---

## Sorun Analizi

Mevcut `compile_shared_library` her çağrıldığında:
1. `tempfile::tempdir()` ile yeni dizin oluşturur
2. `cargo init` + `Cargo.toml` yazar
3. `cargo generate-lockfile` → 221 crate'i crates.io'dan indirir + resolve eder
4. `cargo build --lib` → tüm dependency chain'i sıfırdan derler

**Darboğaz:** Adım 3 (lockfile resolution ~1-2s) ve Adım 4 (compile ~2-3s).

---

## Çözüm 1 — Temp Proje Reuse

**Mevcut:**
```
compile_shared_library() → tempfile::tempdir() → cargo init → cargo build → drop tempdir
```

**Yeni:**
```
compile_shared_library() → mevcut proje dizinini kullan → sadece src/lib.rs güncelle → cargo build
```

### Değişiklikler

**`compiler.rs` — `CompileOptions` genişletme:**
```rust
pub struct CompileOptions {
    pub root: PathBuf,
    pub target_dir: PathBuf,
    pub profile: CompileProfile,
    /// Kalıcı proje dizini (temp yerine). None ise geçici dizin oluşturulur.
    pub project_dir: Option<PathBuf>,
}
```

**`compiler.rs` — `compile_shared_library` mantığı:**
```rust
pub fn compile_shared_library(...) -> Result<CompileResult> {
    let tmp_path = match &options.project_dir {
        Some(dir) => {
            fs::create_dir_all(dir)?;
            dir.clone()
        }
        None => create_temp_project(...)?
    };

    // Her seferinde: sadece src/lib.rs güncelle
    fs::write(tmp_path.join("src/lib.rs"), &lib_rs)?;

    // cargo build
    cargo_build(&tmp_path, ...)?;

    // Bul ve kopyala
    ...
}
```

**`commands.rs` — `bench_reload` ve `dev_server_hot_swap`:**
- İlk çağrıda proje dizinini oluştur
- Sonraki çağrılarda aynı dizini reuse et
- Bench-reload'da `project_dir` option'ını kullan

### Beklenen Kazanç
- İlk compile: ~3-5s (aynı)
- Sonraki compile'lar: ~1-2s (lockfile + incremental)

---

## Çözüm 2 — sccache

**Kurulum:**
```bash
cargo install sccache
```

**CLI'da otomatik etkinleştirme:**
```rust
fn cargo_build_with_sccache(project_dir: &Path, ...) -> Result<bool> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build").arg("--lib");

    // sccache varsa RUSTC_WRAPPER olarak ayarla
    if let Ok(output) = std::process::Command::new("sccache").arg("--version").output() {
        if output.status.success() {
            cmd.env("RUSTC_WRAPPER", "sccache");
        }
    }

    cmd.current_dir(project_dir);
    let status = cmd.status()?;
    Ok(status.success())
}
```

### Beklenen Kazanç
- İlk compile: ~1-2s (cache miss, ama temp reuse sayesinde zaten hızlı)
- Sonraki compile'lar: ~500ms-1s (cache hit)

---

## Adım Sırası

1. `CompileOptions`'a `project_dir: Option<PathBuf>` ekle
2. `compile_shared_library`'da temp/reuse mantığını değiştir
3. `cargo_build_with_sccache` fonksiyonu ekle
4. `bench_reload`'da temp reuse kullan
5. `dev_server_hot_swap`'ta temp reuse kullan
6. Testleri güncelle
7. Benchmark çalıştır

## Doğrulama

1. `cargo test --workspace` → tüm testler yeşil
2. `cargo clippy --workspace` → 0 warning
3. `cargo fmt --check` → temiz
4. `bench-reload` → compile ~500ms-1s hedef
