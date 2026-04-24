# Configuration

```rust,no_run
# use std::path::PathBuf;
# use iced_swdir_tree::{DirectoryFilter, DirectoryTree};
let tree = DirectoryTree::new(PathBuf::from("."))
    .with_filter(DirectoryFilter::AllIncludingHidden)
    .with_max_depth(5);
```

| Method | Purpose |
|---|---|
| `new(root)` | Build a tree rooted at `root`. Only the root is eagerly created. |
| `with_filter(f)` | Builder form of `set_filter`. |
| `with_max_depth(d)` | Refuse to load below depth `d` (0 = root children only). |
| `with_executor(e)` | Route blocking scans through a custom [`ScanExecutor`](custom-executor.md). |
| `with_prefetch_limit(n)` | Enable speculative pre-scanning of folder-children. See [Parallel pre-expansion](prefetch.md). |
| `with_prefetch_skip(iter)` | Replace the default skip list. See [the safety valve](prefetch.md#safety-valve-the-skip-list). |
| `set_filter(f)` | Change the filter at runtime. Re-derives from cache; no I/O. |
| `set_search_query(q)` | Activate [incremental search](incremental-search.md). |
| `clear_search()` | Drop the active search. |
| `handle_key(k, m)` | Translate a keyboard event into a `DirectoryTreeEvent` — see [Keyboard navigation](keyboard-navigation.md). |
| `filter()`, `max_depth()`, `root_path()` | Config accessors. |
| `selected_path()` | Most recently touched path (v0.2 single-select accessor). |
| `selected_paths()` | The full selected set (v0.3 multi-select). |
| `anchor_path()` | Pivot for `SelectionMode::ExtendRange`. |
| `is_selected(path)` | Membership check. |
| `search_query()`, `is_searching()`, `search_match_count()` | Search observers. |
| `is_dragging()`, `drag_sources()`, `drop_target()` | Drag observers. |
