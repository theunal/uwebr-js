# FAZ 20 — Pre-compiled Skeleton + Doğrudan rustc Kullanımı

**Hedef:** Hot swap compile süresini ~1.2s → ~200-400ms'e düşürmek.

**Ön koşul:** FAZ 19 tamamlandı (temp reuse + sccache, ~1.2s).

**Sebep:** FAZ 19'da temp reuse ile ~1.2s'e düştük. Kalan süre:
- rustc crate kontrolü: ~0.5-0.7s
- Windows linker: ~0.3-0.5s

Bu FAZ'da **cargo overhead'ini tamamen kaldırıyoruz**: doğrudan `rustc` çağıracağız.

---

## Mimari

### Mevcut (FAZ 19)
```
hot-swap → write lib.rs → cargo build --lib → .dll
                        cargo overhead: ~0.5s
                        rustc: ~0.5s
                        linker: ~0.3s
                        toplam: ~1.2s
```

### Yeni (FAZ 20)
```
hot-swap → write lib.rs → rustc --edition 2021 -L dep=... lib.rs → .dll
                        cargo overhead: 0s (yok)
                        rustc: ~0.2-0.3s (sadece 1 dosya)
                        linker: ~0.1-0.2s
                        toplam: ~0.3-0.5s
```

---

## Adım 1 — `compiler.rs` yeniden yapılandırma

### 1.1 Skeleton proje Adım 1'de bir kez compile edilir

`compile_shared_library` ilk çağrıldığında:
1. Kalıcı proje dizininde `cargo build --lib` → dependencies cache'lenir
2. `target/debug/deps/` altına `.d` ve `.rlib` dosyaları yazılır
3. Bu dosyalar sonraki rustc çağrıları için kullanılır

### 1.2 Sonraki hot-swap'lar cargo kullanmaz

```
rustc --edition 2021
  --crate-type cdylib
  --crate-name uwebr_dynlib_{name}
  -L dependency=target/debug/deps
  --extern uwebr_core=target/debug/deps/libuwebr_core-XXXX.rlib
  --extern anyhow=target/debug/deps/libanyhow-XXXX.rlib
  ... (tüm dependency'ler)
  -o target/dynlib/uwebr_dynlib_{name}.dll
  src/lib.rs
```

Bu sayede:
- `cargo` hiç çağrılmaz (overhead: 0)
- `rustc` sadece 1 dosya compailer (hızlı)
- Dependency'ler önceden derli, sadece link edilir

---

## Adım 2 — Dependency listesini toplama

`cargo build --lib` çalıştıktan sonra `target/debug/.fingerprint/` dizinindeki bilgilerden hangi `.rlib` dosyalarının gerektiğini tespit edeceğiz.

```rust
fn collect_rlibs(project_dir: &Path, target_dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    // target/debug/deps/ altındaki tüm .rlib dosyalarını bul
    // Her biri için --extern parametresi üret
    // crate ismi → dosya yolu eşleştirmesi
}
```

---

## Adım 3 — `cargo_build_with_sccache` → `compile_with_rustc` dönüşümü

```rust
fn compile_with_rustc(
    project_dir: &Path,
    lib_rs_path: &Path,
    output_path: &Path,
    component_name: &str,
    options: &CompileOptions,
) -> Result<bool> {
    // 1. RLib'leri topla
    let rlibs = collect_rlibs(project_dir, &options.target_dir)?;

    // 2. rustc komutu oluştur
    let mut cmd = Command::new("rustc");
    cmd.arg("--edition").arg("2021")
        .arg("--crate-type").arg("cdylib")
        .arg("--crate-name").arg(format!("uwebr_dynlib_{component_name}"))
        .arg("-o").arg(output_path)
        .arg(lib_rs_path);

    // 3. Her rlib için --extern ekle
    for (name, path) in &rlibs {
        cmd.arg("--extern").arg(format!("{name}={}", path.display()));
    }

    // 4. Dependency dizinini ekle
    let deps_dir = options.target_dir.join("debug").join("deps");
    cmd.arg("-L").arg(&deps_dir);

    // 5. Çalıştır
    let status = cmd.status()?;
    Ok(status.success())
}
```

---

## Adım 4 — İlk build Strategy

İlk build hâlâ `cargo build --lib` ile yapılacak (tüm dependency'leri derlemek için). Sonraki hot-swap'lar rustc kullanacak.

```rust
pub fn compile_shared_library(...) -> Result<CompileResult> {
    let skeleton_built = project_dir.join("target/debug/deps").exists();

    if skeleton_built {
        // Hızlı yol: doğrudan rustc
        compile_with_rustc(project_dir, &lib_rs, &output, component_name, options)?;
    } else {
        // İlk sefer: cargo build (skeleton)
        cargo_build_with_sccache(project_dir, options)?;
    }
    ...
}
```

---

## Adım 5 — Testler

### 5.1 İlk build testi
- İlk `compile_shared_library` çağrısı cargo build kullanmalı
- `target/debug/deps/` dizini oluşmalı

### 5.2 İkinci build testi (hot-swap)
- İkinci çağrı rustc kullanmalı
- Compile süresi ~0.3-0.5s olmalı

### 5.3 RLib toplama testi
- `collect_rlibs` doğru dosyaları bulmalı
- `--extern` parametreleri doğru olmalı

### 5.4 Hata testi
- rustc başarısız olursa fallback cargo build'e dönmeli

---

## Doğrulama

1. `cargo test --workspace` → tüm testler yeşil
2. `cargo clippy --workspace` → 0 warning
3. `cargo fmt --check` → temiz
4. `bench-reload` → compile ~300-500ms hedef
