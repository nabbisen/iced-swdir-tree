# Parallel pre-expansion

Apps on a fast executor (tokio, smol, rayon) usually have more
I/O capacity than one-folder-per-gesture uses.
`with_prefetch_limit(N)` opts into parallel pre-expansion:
whenever a user expands a folder and its children come back, the
widget speculatively fires scan tasks for the first `N` of those
children that are folders. Those scans populate the cache but do
**not** auto-expand anything — `is_loaded = true` without
`is_expanded = true`. When the user later clicks to expand one
of the pre-fetched folders, no I/O happens: it's an instant
fast-path re-expand.

```rust,ignore
use iced_swdir_tree::DirectoryTree;
use std::sync::Arc;

let tree = DirectoryTree::new(root)
    .with_executor(Arc::new(MyTokioExecutor))
    .with_prefetch_limit(10);
```

Pass `0` (or don't call `with_prefetch_limit` at all) to disable
prefetch — that's the default and matches v0.1–0.4 behaviour
exactly. Prefetch is **one level deep**: a folder that loaded
via prefetch does not itself trigger further prefetches, so the
I/O budget is `per_parent` scans per user expansion, not
`per_parent ^ depth`. It also respects `with_max_depth(..)`:
children past the cap are skipped rather than scanned.

Sensible values depend on your executor. On the default
`ThreadExecutor` (one `std::thread::spawn` per scan), keep it
modest (5–25) — each prefetch becomes a real OS thread. On a
bounded tokio/smol pool, a higher value is free: excess tasks
just queue behind the pool's worker cap.

## Safety valve: the skip list

Prefetch never enters directories whose basenames appear in the
widget's skip list. Out of the box the list covers the usual
suspects — version control metadata (`.git`, `.hg`, `.svn`),
JavaScript dependencies (`node_modules`), Python caches and
virtual environments (`__pycache__`, `.venv`, `venv`), Rust/Java
build output (`target`), and generic build output (`build`,
`dist`). The full default is exported as
`iced_swdir_tree::DEFAULT_PREFETCH_SKIP`.

Match is **exact-basename, ASCII case-insensitive** —
`"target"` skips `target/` and `Target/` but not
`my-target-files/`. The skip list governs **prefetch only**; a
user click on a skipped folder still expands it normally.

Extending the default list:

```rust,ignore
use iced_swdir_tree::{DirectoryTree, DEFAULT_PREFETCH_SKIP};

let mut skip: Vec<String> = DEFAULT_PREFETCH_SKIP
    .iter()
    .map(|&s| s.to_string())
    .collect();
skip.push("huge_media_library".into());

let tree = DirectoryTree::new(root)
    .with_prefetch_limit(10)
    .with_prefetch_skip(skip);
```

Disabling the safety valve entirely — for a dedicated `.git`
browser, for example — pass an empty list:

```rust,ignore
let tree = DirectoryTree::new(root)
    .with_prefetch_limit(10)
    .with_prefetch_skip(Vec::<String>::new());
```
