# State machine

This document specifies every valid state transition of the
widget. Each transition takes a current state, applies an event,
and produces a new state and zero or more side effects (async
tasks to schedule).

---

## State dimensions

The widget's state is the product of four independent
dimensions. Each dimension can be in any combination with the
others:

| Dimension | Values |
| --- | --- |
| Loading | Idle (no scan in flight) or Active (one or more scans in flight) |
| Selection | Empty, or a non-empty set of paths |
| Search | Inactive (query = None), or Active (query = Some(…)) |
| Drag | Idle (no drag) or Active (drag in progress) |

"Loading Active" means `tree.generation` has been bumped and at
least one result is pending. Because multiple scans may be
in-flight simultaneously (user expansion + N prefetch scans),
there is no single "the" in-flight scan — just the current
generation value all pending scans were issued with.

---

## Events and transitions

### `Toggled(path: PathBuf)`

**Precondition:** `path` exists in the tree (the node is
reachable from root).

**Case A — path is NOT a directory:**  
No-op. Return immediately without mutating state.

**Case B — path IS a directory AND `is_expanded = true`:**  
*Collapse.*
- Set `node.is_expanded = false`.
- Do NOT bump the generation. Any in-flight scan for a
  descendant remains valid — when it arrives, it will merge
  into the (now-collapsed) node silently. The result is still
  correct: the loaded state is retained in memory; it simply
  isn't drawn.
- **Side effect:** none.

**Case C — path IS a directory AND `is_expanded = false` AND
`is_loaded = true`:**  
*Fast-path expand (data already cached).*
- Set `node.is_expanded = true`.
- No scan required.
- If path was in `prefetching_paths`, remove it (the user
  action supersedes any in-flight prefetch).
- **Side effect:** none.

**Case D — path IS a directory AND `is_expanded = false` AND
`is_loaded = false`:**  
*Slow-path expand (must scan).*
- Check depth cap: if `depth_of(root, path) > config.max_depth`,
  treat as already-loaded and empty (set `is_loaded = true`,
  `children = []`). No side effect.
- Otherwise:
  - Remove path from `prefetching_paths` if present.
  - Bump `tree.generation`.
  - Set `node.is_expanded = true`.
  - **Side effect:** issue async scan for `path` tagged with
    the new generation.

---

### `Loaded(payload: LoadPayload)`

```
LoadPayload {
    path:       PathBuf
    generation: u32
    depth:      u32
    result:     Result<Vec<LoadedEntry>, Error>
}
```

**Step 1 — Staleness check:**  
If `payload.generation != tree.generation`, discard silently.
Return with no state changes and no side effects.

**Step 2 — Find the node:**  
Look up `path` in the tree. If not found (rare: widget was
rebuilt or root changed), discard silently.

**Step 3 — Merge result:**  
- If `result` is `Ok(entries)`:
  - `node.children = filter(entries, config.filter)`
  - `node.error = None`
- If `result` is `Err(e)`:
  - `node.children = []`
  - `node.error = Some(e)`
- Set `node.is_loaded = true`.

**Step 4 — Update cache:**  
If `result` is `Ok`, put `(generation, raw_entries)` into
`cache[path]`. "Raw" means unfiltered — the cache always stores
the complete entry list.

**Step 5 — Sync selection flags:**  
Walk the whole tree, clear every `node.is_selected`, then
re-set `is_selected = true` for every node whose path is in
`selected_paths`. This is O(N_loaded) but called only when
new data arrives.

**Step 6 — Recompute search visibility:**  
If `search` is `Some`, run `walk_for_search` over the full
tree (O(N_loaded)) and update `search.visible_paths` and
`search.match_count`.

**Step 7 — Prefetch cascade check:**  
- If `path` is in `prefetching_paths`:
  - Remove path from `prefetching_paths`.
  - No further prefetch. Return no side effects.
- Else (user-initiated scan):
  - Compute prefetch targets = folder-children of `path` that
    are not loaded, not in `prefetch_skip`, within `max_depth`,
    up to `prefetch_per_parent` of them.
  - For each target T:
    - Insert T into `prefetching_paths`.
    - Bump `tree.generation`.
    - **Side effect:** issue async scan for T with new generation.

---

### `Selected(path: PathBuf, is_dir: bool, mode: SelectionMode)`

**Common to all modes:**
- Set `tree.active_path = Some(path.clone())`.
- After mutating `selected_paths`, call `sync_selection_flags`.

**`SelectionMode::Replace`:**
1. `selected_paths = vec![path]`.
2. `tree.anchor_path = Some(path)`.

**`SelectionMode::Toggle`:**
1. If `path` is in `selected_paths`: remove it.
   Else: append it.
2. `tree.anchor_path = Some(path)`.

**`SelectionMode::ExtendRange`:**
1. If `anchor_path` is `None`, fall back to `Replace`.
2. Otherwise:
   - Collect `visible_rows()` (the current ordered list of
     drawn rows, respecting expansion and search).
   - Find the index of `anchor_path` and the index of `path`.
   - The range is the slice `[min_idx .. max_idx]` inclusive.
   - `selected_paths = all paths in that range`.
3. `tree.active_path = Some(path)`. `anchor_path` unchanged.

No side effects for any `SelectionMode`.

---

### Drag transitions

#### `Drag::Pressed(path: PathBuf, is_dir: bool)`

- If `path ∈ selected_paths`: `sources = selected_paths.clone()`.
- Else: `sources = vec![path]`.
- `tree.drag = Some(DragState { sources, hovered_target: None,
  started_at: path })`.

No side effects.

#### `Drag::Entered(path: PathBuf)`

A row received the mouse hover during an active drag.

- If no drag is active: no-op.
- Check validity: `path` is a valid drop target iff:
  1. `path` is a directory.
  2. `path` is NOT one of the drag sources.
  3. `path` is NOT a descendant of any drag source.
- If valid: `drag.hovered_target = Some(path)`.
- Else: `drag.hovered_target = None`.

No side effects.

#### `Drag::Exited(path: PathBuf)`

The mouse left a row during drag.

- If `drag.hovered_target == Some(path)`, clear it:
  `drag.hovered_target = None`.
- (The path argument is used to avoid clearing a target that was
  already overwritten by a Entered for a different row.)

No side effects.

#### `Drag::Released(path: PathBuf)`

Mouse button released over a row.

- If `path == drag.started_at` (click, not drag):
  - Clear drag state: `tree.drag = None`.
  - **Side effect:** emit a deferred `Selected(path, is_dir,
    Replace)` event. (The application will receive it on the
    next event cycle.)
- Else (genuine drop):
  - Capture `sources = drag.sources.clone()`,
    `destination = path`.
  - Clear drag state: `tree.drag = None`.
  - **Side effect:** emit `DragCompleted { sources, destination }`.

#### `Drag::Cancelled`

Explicit cancellation (e.g., Escape key).

- If no drag active: no-op.
- Clear drag state: `tree.drag = None`.

No side effects.

---

### `set_filter(filter: DisplayFilter)`

- If `filter == config.filter`: no-op.
- Set `config.filter = filter`.
- Rebuild the visible child list for every loaded node from
  the cache. This is a pure re-derivation: for each path P
  in `cache`, `node.children = cache[P].entries.filter(config.filter)`.
- Call `sync_selection_flags()`.
- Call `recompute_search_visibility()` if search is active.

No side effects (no scans issued).

---

### `set_search_query(query: String)`

- If `query.is_empty()` → equivalent to `clear_search()`.
- Set `search = Some(SearchState::new(query))`.
- Call `recompute_search_visibility()`.

`recompute_search_visibility()`:
1. Walk the entire loaded tree, ignoring `is_expanded`.
2. For each node, check `basename(node.path).to_lowercase()
   .contains(query_lower)`.
3. A node is "visible" if it matches OR any descendant matches.
4. Populate `search.visible_paths` and `search.match_count`.

No side effects.

---

### `clear_search()`

- Set `search = None`.

No side effects.

---

## The `visible_rows()` function

This function is called by the view layer and the keyboard
navigation system to determine the ordered list of rows
currently drawn.

**When search is inactive:**  
Depth-first pre-order walk of the tree:
- Visit root.
- For each child: skip if `filter` would hide it. Recurse
  if `node.is_dir && node.is_expanded && node.is_loaded`.

**When search is active:**  
Depth-first pre-order walk, but using `search.visible_paths`
as the gating set instead of `is_expanded`:
- Visit a node iff `node.path ∈ visible_paths`.
- Always descend into children (bypassing `is_expanded`) —
  search "sees through" collapse.

The returned list is a slice of `(node_ref, depth)` pairs in
draw order. Both the view (for rendering row positions) and
keyboard navigation (for `↑` / `↓` / `Home` / `End` movement)
operate over the same list, ensuring they never diverge.

---

## Composability rules

Because the four state dimensions are orthogonal, every
combination is valid:

| Situation | Behaviour |
| --- | --- |
| Search active while drag in progress | Visible rows are search-filtered; drag-source rows may be invisible but the drag is still valid. Drop targets are constrained to the visible set. |
| Search active while scan in progress | When scan completes, `recompute_search_visibility()` runs, potentially revealing new matches from the just-loaded data. |
| Drag in progress while filter changes | `set_filter` rebuilds nodes but does not clear drag state. The drag continues over the new filtered view. |
| Collapse while search active | The view respects `visible_paths` for rendering (bypassing `is_expanded`), so a collapsed folder whose descendants match still shows in search mode. |
| Multi-select while search active | `ExtendRange` uses `visible_rows()` which is search-aware; the range covers only visible rows. |
