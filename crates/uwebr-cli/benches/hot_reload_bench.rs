use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::fs;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Sample .uwebr content of varying sizes for realistic benchmarks
// ---------------------------------------------------------------------------

const SMALL_UWEBR: &str = r#"<div class="app">
  <h1>Hello</h1>
  <p>Count: {count}</p>
  <button on:click={increment}>+1</button>
</div>

<script>
  let count = 0;
  function increment() { count += 1; }
</script>

<style>
  .app { font-family: sans-serif; padding: 2rem; }
  h1 { color: #333; font-size: 2rem; }
  button { background: #007bff; color: white; padding: 0.5rem 1rem; }
</style>
"#;

const MEDIUM_UWEBR: &str = r#"<div class="dashboard">
  <header class="header">
    <h1>{title}</h1>
    <nav>
      <a on:click={showTab('home')}>Home</a>
      <a on:click={showTab('settings')}>Settings</a>
    </nav>
  </header>
  <main class="content">
    {#each items as item}
      <div class="card">
        <h3>{item.name}</h3>
        <p>{item.description}</p>
        <span class="badge">{item.count}</span>
      </div>
    {/each}
  </main>
  <footer class="footer">
    <p>Status: {status}</p>
    <button on:click={refresh}>Refresh</button>
  </footer>
</div>

<script>
  let title = 'Dashboard';
  let status = 'ready';
  let currentTab = 'home';
  let items = [];

  function showTab(name) { currentTab = name; }
  function refresh() { status = 'refreshing...'; }
</script>

<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  .dashboard { font-family: system-ui; min-height: 100vh; display: flex; flex-direction: column; }
  .header { background: #1a1a2e; color: white; padding: 1rem 2rem; display: flex; justify-content: space-between; align-items: center; }
  .header h1 { font-size: 1.5rem; }
  .header nav a { color: #e0e0e0; margin-left: 1rem; cursor: pointer; text-decoration: none; }
  .header nav a:hover { color: #007bff; }
  .content { flex: 1; padding: 2rem; display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 1.5rem; }
  .card { background: white; border-radius: 8px; padding: 1.5rem; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
  .card h3 { color: #1a1a2e; margin-bottom: 0.5rem; }
  .card p { color: #666; line-height: 1.5; }
  .badge { display: inline-block; background: #007bff; color: white; padding: 0.25rem 0.75rem; border-radius: 12px; font-size: 0.875rem; margin-top: 0.5rem; }
  .footer { background: #f5f5f5; padding: 1rem 2rem; display: flex; justify-content: space-between; align-items: center; border-top: 1px solid #e0e0e0; }
  .footer button { background: #28a745; color: white; border: none; padding: 0.5rem 1.5rem; border-radius: 4px; cursor: pointer; }
</style>
"#;

const LARGE_UWEBR: &str = r#"<div class="app">
  <aside class="sidebar">
    <div class="logo">Logo</div>
    <nav class="nav">
      {#each navItems as item}
        <a class="nav-item" on:click={navigate(item.id)}>{item.label}</a>
      {/each}
    </nav>
  </aside>
  <div class="main">
    <header class="topbar">
      <input type="search" placeholder="Search..." on:input={onSearch} value={query} />
      <div class="user-info">
        <span>{userName}</span>
        <button on:click={logout}>Logout</button>
      </div>
    </header>
    <section class="content">
      {#if loading}
        <div class="spinner"></div>
      {:else}
        {#each rows as row}
          <div class="table-row">
            <span class="col-id">{row.id}</span>
            <span class="col-name">{row.name}</span>
            <span class="col-email">{row.email}</span>
            <span class="col-status">{row.status}</span>
            <div class="col-actions">
              <button on:click={edit(row.id)}>Edit</button>
              <button on:click={deleteRow(row.id)}>Delete</button>
            </div>
          </div>
        {/each}
      {/if}
    </section>
    <footer class="pagination">
      <button on:click={prevPage}>← Prev</button>
      <span>Page {page} of {totalPages}</span>
      <button on:click={nextPage}>Next →</button>
    </footer>
  </div>
</div>

<script>
  let navItems = [{id: 'home', label: 'Home'}, {id: 'users', label: 'Users'}, {id: 'settings', label: 'Settings'}];
  let userName = 'Admin';
  let query = '';
  let loading = false;
  let rows = [];
  let page = 1;
  let totalPages = 10;

  function navigate(id) { loading = true; }
  function onSearch(e) { query = e.target.value; }
  function logout() { }
  function edit(id) { }
  function deleteRow(id) { }
  function prevPage() { if (page > 1) page -= 1; }
  function nextPage() { if (page < totalPages) page += 1; }
</script>

<style>
  :root { --sidebar-width: 260px; --topbar-height: 56px; --primary: #4f46e5; --danger: #dc2626; --text: #1f2937; --bg: #f9fafb; }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  .app { display: flex; height: 100vh; font-family: system-ui, -apple-system, sans-serif; color: var(--text); }
  .sidebar { width: var(--sidebar-width); background: #111827; color: white; display: flex; flex-direction: column; }
  .logo { padding: 1.25rem; font-size: 1.25rem; font-weight: 700; border-bottom: 1px solid #374151; }
  .nav { flex: 1; padding: 0.5rem 0; }
  .nav-item { display: block; padding: 0.75rem 1.25rem; color: #d1d5db; text-decoration: none; cursor: pointer; transition: background 0.15s; }
  .nav-item:hover { background: #1f2937; color: white; }
  .main { flex: 1; display: flex; flex-direction: column; background: var(--bg); }
  .topbar { height: var(--topbar-height); background: white; border-bottom: 1px solid #e5e7eb; display: flex; align-items: center; justify-content: space-between; padding: 0 1.5rem; }
  .topbar input { width: 300px; padding: 0.5rem 0.75rem; border: 1px solid #d1d5db; border-radius: 6px; font-size: 0.875rem; }
  .user-info { display: flex; align-items: center; gap: 0.75rem; }
  .user-info button { background: none; border: 1px solid #d1d5db; padding: 0.375rem 0.75rem; border-radius: 4px; cursor: pointer; font-size: 0.8125rem; }
  .content { flex: 1; overflow-y: auto; padding: 1.5rem; }
  .spinner { width: 32px; height: 32px; border: 3px solid #e5e7eb; border-top-color: var(--primary); border-radius: 50%; animation: spin 0.6s linear infinite; margin: 2rem auto; }
  .table-row { display: grid; grid-template-columns: 60px 1fr 1fr 100px 140px; align-items: center; padding: 0.75rem 1rem; background: white; border: 1px solid #e5e7eb; border-radius: 6px; margin-bottom: 0.5rem; }
  .col-id { font-weight: 600; color: #6b7280; }
  .col-status { text-transform: capitalize; }
  .col-actions { display: flex; gap: 0.5rem; }
  .col-actions button { padding: 0.25rem 0.5rem; border: none; border-radius: 4px; cursor: pointer; font-size: 0.75rem; }
  .col-actions button:first-child { background: #dbeafe; color: #1d4ed8; }
  .col-actions button:last-child { background: #fee2e2; color: var(--danger); }
  .pagination { display: flex; align-items: center; justify-content: center; gap: 1rem; padding: 1rem; border-top: 1px solid #e5e7eb; background: white; }
  .pagination button { padding: 0.375rem 0.75rem; border: 1px solid #d1d5db; border-radius: 4px; background: white; cursor: pointer; }
</style>
"#;

const CSS_ONLY: &str = r#".app { font-family: sans-serif; padding: 2rem; }
h1 { color: #333; }
button { background: #007bff; color: white; }
"#;

const CSS_MEDIUM: &str = r#":root { --primary: #4f46e5; --text: #1f2937; }
* { margin: 0; padding: 0; box-sizing: border-box; }
.app { font-family: system-ui; min-height: 100vh; display: flex; flex-direction: column; }
.header { background: #1a1a2e; color: white; padding: 1rem 2rem; display: flex; justify-content: space-between; }
.header h1 { font-size: 1.5rem; }
.header nav a { color: #e0e0e0; margin-left: 1rem; }
.content { flex: 1; padding: 2rem; display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 1.5rem; }
.card { background: white; border-radius: 8px; padding: 1.5rem; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
.badge { background: var(--primary); color: white; padding: 0.25rem 0.75rem; border-radius: 12px; }
.footer { background: #f5f5f5; padding: 1rem 2rem; border-top: 1px solid #e0e0e0; }
"#;

const CSS_LARGE: &str = r#":root { --sidebar-width: 260px; --topbar-height: 56px; --primary: #4f46e5; --danger: #dc2626; --text: #1f2937; --bg: #f9fafb; }
* { margin: 0; padding: 0; box-sizing: border-box; }
.app { display: flex; height: 100vh; font-family: system-ui, -apple-system, sans-serif; color: var(--text); }
.sidebar { width: var(--sidebar-width); background: #111827; color: white; display: flex; flex-direction: column; }
.logo { padding: 1.25rem; font-size: 1.25rem; font-weight: 700; border-bottom: 1px solid #374151; }
.nav { flex: 1; padding: 0.5rem 0; }
.nav-item { display: block; padding: 0.75rem 1.25rem; color: #d1d5db; text-decoration: none; cursor: pointer; }
.nav-item:hover { background: #1f2937; color: white; }
.main { flex: 1; display: flex; flex-direction: column; background: var(--bg); }
.topbar { height: var(--topbar-height); background: white; border-bottom: 1px solid #e5e7eb; display: flex; align-items: center; padding: 0 1.5rem; }
.topbar input { width: 300px; padding: 0.5rem 0.75rem; border: 1px solid #d1d5db; border-radius: 6px; }
.content { flex: 1; overflow-y: auto; padding: 1.5rem; }
.table-row { display: grid; grid-template-columns: 60px 1fr 1fr 100px 140px; padding: 0.75rem 1rem; background: white; border: 1px solid #e5e7eb; border-radius: 6px; margin-bottom: 0.5rem; }
.pagination { display: flex; justify-content: center; gap: 1rem; padding: 1rem; border-top: 1px solid #e5e7eb; }
"#;

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_classify_changes(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_reload/classify_changes");

    let css_only = vec![PathBuf::from("src/styles.css")];
    let full = vec![PathBuf::from("src/App.uwebr")];
    let mixed = vec![
        PathBuf::from("src/App.uwebr"),
        PathBuf::from("src/theme.css"),
    ];
    let none = vec![PathBuf::from("README.md")];

    group.bench_function("css_only", |b| {
        b.iter(|| uwebr_cli::commands::classify_changes(black_box(&css_only)))
    });
    group.bench_function("full", |b| {
        b.iter(|| uwebr_cli::commands::classify_changes(black_box(&full)))
    });
    group.bench_function("mixed", |b| {
        b.iter(|| uwebr_cli::commands::classify_changes(black_box(&mixed)))
    });
    group.bench_function("none", |b| {
        b.iter(|| uwebr_cli::commands::classify_changes(black_box(&none)))
    });

    group.finish();
}

fn bench_transpile(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_reload/transpile");

    for (name, content) in [
        ("small", SMALL_UWEBR),
        ("medium", MEDIUM_UWEBR),
        ("large", LARGE_UWEBR),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &content,
            |b, &content| {
                b.iter(|| uwebr_cli::transpiler::transpile(black_box(content), black_box("App")))
            },
        );
    }

    group.finish();
}

fn bench_css_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_reload/css_parse");

    for (name, css) in [
        ("small", CSS_ONLY),
        ("medium", CSS_MEDIUM),
        ("large", CSS_LARGE),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(name), &css, |b, &css| {
            b.iter(|| uwebr_render::stylebook::StyleBook::parse(black_box(css)))
        });
    }

    group.finish();
}

fn bench_css_reparse(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_reload/css_reparse");
    let vw = 800.0;
    let vh = 600.0;

    for (name, css) in [
        ("small", CSS_ONLY),
        ("medium", CSS_MEDIUM),
        ("large", CSS_LARGE),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(name), &css, |b, &css| {
            b.iter_batched(
                || {
                    // Cold parse first so reparse has a populated StyleBook to mutate
                    uwebr_render::stylebook::StyleBook::parse_vp(css, vw, vh).unwrap()
                },
                |mut book| {
                    black_box(book.reparse(css, vw, vh)).unwrap();
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_build_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_reload/build_cache");
    group.sample_size(10);

    // Create a temp dir with .uwebr files to benchmark real file I/O
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // Write scaffold .uwebr files
    fs::write(root.join("App.uwebr"), SMALL_UWEBR).unwrap();
    fs::write(root.join("Dashboard.uwebr"), MEDIUM_UWEBR).unwrap();
    fs::write(root.join("Admin.uwebr"), LARGE_UWEBR).unwrap();

    group.bench_function("build_all_3_files", |b| {
        b.iter(|| {
            let mut cache = uwebr_cli::commands::BuildCache::new(root.to_path_buf());
            black_box(cache.build_all()).unwrap();
        })
    });

    group.bench_function("build_incremental_1_file", |b| {
        b.iter_batched(
            || {
                let mut cache = uwebr_cli::commands::BuildCache::new(root.to_path_buf());
                cache.build_all().unwrap();
                cache
            },
            |mut cache| {
                let changed = vec![root.join("Dashboard.uwebr")];
                black_box(cache.build_incremental(&changed)).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_classify_changes,
    bench_transpile,
    bench_css_parse,
    bench_css_reparse,
    bench_build_cache,
);
criterion_main!(benches);
