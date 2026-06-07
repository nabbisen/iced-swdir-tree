# Data model

This document describes every piece of state the widget holds,
its type shape, and the invariants that hold at all times.

## Overview diagram

```
DirectoryTree
├── root: TreeNode                  ← always present
├── config: TreeConfig
│   ├── root_path: PathBuf
│   ├── filter: DisplayFilter       ← FoldersOnly | FilesAndFolders | AllIncludingHidden
│   ├── max_depth: Option<u32>
│   ├── prefetch_per_parent: usize  ← 0 = disabled
│   └── prefetch_skip: Vec<String>  ← basename exact-match skip list
├── cache: TreeCache                ← path → (generation, entries[])
├── generation: u32                 ← monotonically increasing, wraps on overflow
├── selected_paths: Vec<PathBuf>    ← authoritative selection set
├── active_path: Option<PathBuf>    ← most-recently-touched path
├── anchor_path: Option<PathBuf>    ← Shift-range pivot
├── drag: Option<DragState>         ← None when no drag in progress
├── prefetching_paths: Set<PathBuf> ← in-flight prefetch scan targets
├── search: Option<SearchState>     ← None when search inactive
└── icon_theme: Arc<dyn IconTheme>  ← always Some (defaults to stock)
```

---

## TreeNode

Each directory entry in the tree is represented as a recursive
value:

```
TreeNode {
    path:        PathBuf       ← absolute path
    is_dir:      bool
    is_expanded: bool          ← true iff user has opened this folder
    is_loaded:   bool          ← true iff children[] has been populated by a scan
    is_selected: bool          ← derived from selected_paths; view hint only
    children:    Vec<TreeNode> ← empty until is_loaded
    error:       Option<Error> ← set when a scan returned Err (permission denied, etc.)
}
```

### Invariants on TreeNode

1. **`is_expanded` ⟹ `is_dir`.**  
   Only directories can be expanded. Files always have
   `is_expanded = false`.

2. **`is_expanded` and `is_loaded` are independent.**  
   A node may be loaded-but-collapsed (common after prefetch).
   A node may be expanded-but-not-loaded (briefly during an
   in-flight scan — the view draws a loading indicator).

3. **`children` is empty iff `!is_loaded`**, with one exception:  
   A directory that genuinely contains zero entries (empty
   folder, or all entries filtered out) will have
   `is_loaded = true` and `children = []`.

4. **`error.is_some()` ⟹ `is_loaded = true`, `children = []`.**  
   An error from `scan_dir` (permission denied, path vanished)
   is stored on the node. The view renders it in a greyed-out
   error state. The children list is always empty when an error
   is present.

5. **`is_selected` is a derived view hint.**  
   It is set by calling `sync_selection_flags()` after any
   mutation that rebuilds nodes. It is not authoritative —
   `selected_paths` on the root is authoritative. Any code
   that needs to know "is this path selected?" should query
   `selected_paths.contains(&path)`, not `node.is_selected`.

---

## TreeConfig

Configuration that is set at construction and may be mutated
by the application at runtime:

```
TreeConfig {
    root_path:           PathBuf
    filter:              DisplayFilter
    max_depth:           Option<u32>     ← None = unbounded
    prefetch_per_parent: usize           ← 0 = disabled
    prefetch_skip:       Vec<String>     ← default: DEFAULT_PREFETCH_SKIP
}
```

`max_depth` is measured relative to the root. `Some(0)` means
only root's direct children are ever loaded.

`prefetch_skip` entries are matched against `path.file_name()`
using ASCII case-insensitive exact comparison. A folder whose
basename matches any entry is excluded from prefetch target
selection. The skip list never prevents user-initiated scans.

---

## TreeCache

A flat map from `PathBuf` to `(generation: u32, entries:
Vec<LoadedEntry>)`. Holds the raw, **unfiltered** results of
every scan that has completed with a matching generation.

```
TreeCache: Map<PathBuf, (u32, Vec<LoadedEntry>)>

LoadedEntry {
    path:   PathBuf
    is_dir: bool
    is_hidden: bool   ← OS-native: dotfile on Unix, HIDDEN attr on Windows
}
```

### Why cache at all?

When the application calls `set_filter(new_filter)`, the widget
must immediately derive a new child list for every loaded folder,
without issuing new scans. The cache stores the full (unfiltered)
entry list, so filter changes are computed in memory with zero
I/O.

### Cache invalidation

The cache is never explicitly invalidated in normal use. Old
entries are simply overwritten when a newer scan for the same
path completes.

---

## Generation counter

```
generation: u32   ← monotonically increasing; wraps silently on overflow
```

Incremented **before** issuing a new scan task:

```
// pseudocode
tree.generation = tree.generation.wrapping_add(1)
let g = tree.generation
spawn_blocking(|| scan_dir(path))
  .on_complete(|result| on_loaded(LoadPayload { path, generation: g, result }))
```

Each prefetch scan increments the generation independently —
each gets its own generation value.

**Acceptance condition:**  
```
if payload.generation != tree.generation { return; }  // discard stale
```

Because the counter wraps on overflow, the acceptance condition
is strict equality, not `>=`. The chance of a wrap-and-coincide
collision is negligible in practice.

**What bumps the generation:**
- User-initiated folder expansion (Toggled event).
- Each prefetch scan issued after a scan completes.

**What does NOT bump the generation:**
- `set_filter` — filter changes use the cache.
- `set_search_query` — search is purely in-memory.
- Selection changes.
- Drag state changes.
- Collapse (Toggled when already expanded).

---

## Selection state

Three related fields:

```
selected_paths: Vec<PathBuf>     ← the source of truth
active_path:    Option<PathBuf>  ← most-recently-touched path
anchor_path:    Option<PathBuf>  ← Shift-range pivot
```

### `selected_paths`

The ordered list of currently selected paths. "Ordered" means
insertion-ordered, not path-sorted — the order in which the
user selected them. This is the value returned by
`selected_paths()` and is the only source consulted for
`is_selected` queries.

A path may be in `selected_paths` even if its `TreeNode` does
not currently exist in memory (the folder was not yet expanded,
or the node was filtered out). The path remains selected through
filter changes, re-scans, and search.

Invariant: no duplicates. Each path appears at most once.

### `active_path`

The path that was the target of the most recent explicit
selection action, regardless of `SelectionMode`. Updated by:
- `SelectionMode::Replace` — set to new path.
- `SelectionMode::Toggle` — set to new path.
- `SelectionMode::ExtendRange` — set to new path (range end).

Used by the view to know which row should receive focus styling.

### `anchor_path`

The pivot for `SelectionMode::ExtendRange` (Shift-click).
Updated by:
- `SelectionMode::Replace` — set to new path.
- `SelectionMode::Toggle` — set to new path.

**Not** updated by `SelectionMode::ExtendRange` — the anchor
stays fixed while the user extends the range. Selecting A
(Replace), then B (ExtendRange) → anchor=A, range=[A…B]. Then
C (ExtendRange) → anchor=A, range=[A…C], B reverts to
unselected if C is on the other side. Standard list-range
selection.

---

## Drag state

```
DragState {
    sources:         Vec<PathBuf>     ← paths being dragged
    hovered_target:  Option<PathBuf>  ← current valid drop target, if any
    started_at:      PathBuf          ← path where mouse was pressed
}
```

`drag: Option<DragState>` — `None` means no drag is active.

`sources` is populated at drag-start: if `started_at` is in
`selected_paths`, then `sources = selected_paths.clone()`.
Otherwise `sources = vec![started_at]`.

`hovered_target` is updated as the mouse moves over rows:
- `None` if no valid target is hovered.
- `Some(path)` only for folder rows that pass the valid-target
  check. A valid target must: be a directory, not be a source
  path or a descendant of any source path.

---

## Prefetch state

```
prefetching_paths: HashSet<PathBuf>
```

Tracks which paths currently have an in-flight prefetch scan.
When a prefetch scan is issued for path P, P is inserted into
this set. When the scan result for P arrives in `on_loaded`,
P is removed from the set. This one-level registry prevents
cascade: a scan result whose path is in `prefetching_paths` is
treated as a prefetch completion and never triggers a new wave
of prefetches from its own children.

If the user explicitly clicks to expand a path P while a
prefetch for P is in flight:
1. P is removed from `prefetching_paths`.
2. A new user-initiated scan is issued for P with a fresh
   generation.
3. The original prefetch result will arrive with a stale
   generation and be discarded.
4. The user-initiated result arrives, merges, and (since P is
   no longer in `prefetching_paths`) triggers prefetch of P's
   children as normal.

---

## Search state

```
SearchState {
    query:         String             ← as provided by the application; original casing
    query_lower:   String             ← query.to_lowercase(); used for comparisons
    visible_paths: HashSet<PathBuf>   ← match set ∪ ancestor set; recomputed on change
    match_count:   usize              ← direct matches only; ancestors not counted
}

search: Option<SearchState>           ← None when inactive
```

`visible_paths` satisfies: a path P is in the set iff P
matches the query by basename substring **or** some descendant
of P matches.

`match_count` counts only direct matches, not ancestors.
This is what apps show in their "N results" display.

The set is recomputed from scratch (O(N_loaded)) whenever:
- `set_search_query` is called.
- `set_filter` is called (filter may expose or hide matches).
- A scan completes via `on_loaded` (new children might match).

---

## Icon theme

```
icon_theme: Arc<dyn IconTheme>
```

Always present. Default selected at construction:
- With `icons` feature: `LucideTheme`
- Without `icons` feature: `UnicodeTheme`

The theme is consulted during view rendering to convert an
`IconRole` to an `IconSpec { glyph, font, size }`. It is not
consulted during state transitions — it is purely a rendering
concern.

Because it is `Arc<dyn>`, the theme can be cloned cheaply and
shared across frames.
