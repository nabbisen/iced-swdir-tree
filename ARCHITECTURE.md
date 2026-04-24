# Architecture

```text
src/
  lib.rs                       # Re-exports and crate docs
  directory_tree.rs            # State struct and builder methods
  directory_tree/
    config.rs                  # DirectoryFilter, TreeConfig
    config/tests.rs
    drag.rs                    # DragMsg + DragState state machine (v0.4)
    drag/tests.rs
    error.rs                   # Crate Error type
    executor.rs                # ScanExecutor trait, ThreadExecutor default
    icon.rs                    # Feature-gated icon renderer (lucide / text)
    keyboard.rs                # handle_key adapter + bindings
    keyboard/tests.rs
    message.rs                 # DirectoryTreeEvent + LoadPayload
    node.rs                    # TreeNode, LoadedEntry, TreeCache, VisibleRow
    node/tests.rs
    selection.rs               # SelectionMode enum + from_modifiers
    selection/tests.rs
    update.rs                  # update() dispatcher + depth_of helper
    update/
      on_toggled.rs            # expand/collapse handler (v0.4.1 split)
      on_selected.rs           # SelectionMode handler + range helper
      on_drag.rs               # drag state machine handler
      on_loaded.rs             # async-scan merge + build_children
      tests.rs
    view.rs                    # Render function for view()
    walker.rs                  # async scan wrapper + normalization
    walker/tests.rs
```

The public API is intentionally small. The internal layering
separates state ownership (`directory_tree.rs`), events (`message.rs`),
state transitions (`update.rs` and its `update/on_xxx.rs`
submodules — one per event variant, added in v0.4.1), rendering
(`view.rs`), data access (`walker.rs`), blocking-work dispatch
(`executor.rs`), keyboard translation (`keyboard.rs`), selection
modes (`selection.rs`), and drag-and-drop state (`drag.rs`), which
makes room for the remaining v1.0 roadmap items (parallel
pre-expand, incremental search, icon themes) without touching the
widget surface.

## Selection model (v0.3+)

Three fields on `DirectoryTree` encode the selected set:

| Field            | Role |
|------------------|------|
| `selected_paths: Vec<PathBuf>` | The authoritative set of selected paths. Order is not semantic. |
| `active_path: Option<PathBuf>` | The most recently touched path (last click, last Space-toggle, last `ExtendRange` target). `selected_path()` returns this — preserving the v0.2 single-select accessor semantics. |
| `anchor_path: Option<PathBuf>` | The pivot for `SelectionMode::ExtendRange`. Updated by `Replace` and `Toggle`; **not** updated by `ExtendRange` itself — successive Shift+clicks all extend from the same origin, matching Windows Explorer / macOS Finder / VS Code. |

The per-node `TreeNode::is_selected` flag is a view-layer cache
re-synced from `selected_paths` by
`DirectoryTree::sync_selection_flags()` after any mutation that
replaces node instances. This keeps filter changes and subtree
reloads lossless: every selected path survives because the cache
gets re-derived from the authoritative set.

### Mode semantics

`SelectionMode::from_modifiers(m)` maps iced `Modifiers` into one of
three variants:

- `Replace` (no modifier): clears the set, selects just the target.
  Updates both `active_path` and `anchor_path`.
- `Toggle` (Ctrl/Cmd): adds the target if absent, removes it if
  present. Updates both `active_path` and `anchor_path`.
- `ExtendRange` (Shift): replaces the set with every visible row
  between `anchor_path` and the target, in render order. Updates
  only `active_path`; the anchor stays put so further Shift+clicks
  remain anchored. Falls back to `Replace` semantics if no anchor
  is set or if either endpoint is not currently visible (filtered
  out, ancestor collapsed, not yet loaded).

### View-level click modifiers

iced 0.14's `button::on_press` can't observe modifier keys, so the
built-in view emits `SelectionMode::Replace` unconditionally.
Applications that want multi-select track modifier state separately
(via a `keyboard::listen()` subscription) and rewrite the mode in
their own update handler before forwarding to `tree.update` — the
`examples/multi_select.rs` example demonstrates the full pattern.
Keyboard bindings (`handle_key`) produce the right mode directly
because key events carry modifiers at press time.

## Scan dispatch

`walker::scan` is runtime-agnostic: it produces an `iced::Task`
backed by a `ScanFuture`, itself obtained from
`Arc<dyn ScanExecutor>`. The default `ThreadExecutor` spawns one
`std::thread` per scan, which is correct but slightly wasteful for
apps that already run a blocking-task pool. Those apps can
implement `ScanExecutor` (one method, `spawn_blocking`) and swap
it in at construction time via `DirectoryTree::with_executor`.

## Drag-and-drop (v0.4+)

`drag.rs` holds a small finite-state machine. In state `Idle` the
widget has no active drag; `drag` on `DirectoryTree` is `None`. A
press on a row transitions to `Dragging { sources, primary,
primary_is_dir, hover }` where `sources` is the current selected
set if the pressed row is in it, otherwise just the pressed row —
matching Explorer/Finder behaviour. `Entered`/`Exited` events
update `hover` (only if the incoming path is a valid drop target
per the three rules in `DragState::is_valid_target`). `Released`
inspects the state: same row as `primary` → emit a delayed
`Selected(..., Replace)` via `Task::done` (this is the
"deferred selection" pattern that makes multi-item drag possible);
different row with a valid `hover` → emit `DragCompleted { sources,
destination }`; anything else → clear state silently. `Cancelled`
(from Escape or an app-initiated abort) clears state unconditionally.

The widget never performs a filesystem operation; `DragCompleted`
is the app's cue to act. This keeps the widget reusable for
non-local backends (zip archives, network mounts, abstract
hierarchies) without dragging in I/O assumptions.

View-layer row hitboxes switched from `button` to
`mouse_area(container)` in v0.4. The reason is that iced 0.14's
`button::on_press` fires only on click-completion, so there's no
way to observe mouse-down as a distinct event from mouse-up. A
`mouse_area` exposes `on_press` / `on_release` / `on_enter` /
`on_exit` separately, which is exactly what the state machine
needs. The cost is that the plain container doesn't track hover
state the way `button::text` did — non-selected rows lose their
native hover glow. Selected and drop-target rows still paint with
styled backgrounds from the theme's extended palette.

## Parallel pre-expansion (v0.5+)

Pre-fetch is implemented entirely inside the update layer. Three
things coordinate:

1. **`config.prefetch_per_parent`** — the cap. `0` disables.
2. **`on_loaded` returns `Vec<PathBuf>`** of prefetch targets
   computed from the just-merged children (`select_prefetch_targets`
   filters to folder-children that aren't already loaded, under
   `max_depth`, capped at `per_parent`).
3. **`prefetching_paths: HashSet<PathBuf>`** on the tree — records
   which paths are currently being prefetch-scanned. When the
   scan result for one of them arrives, `on_loaded` drains the
   flag and returns an **empty** target list — no cascade. A
   user-initiated `on_toggled` removes any pending entry for the
   path it's about to scan, so the stale prefetch result (dropped
   by generation mismatch) won't leave dangling state.

The dispatcher is the only layer that knows about the executor:
it takes the `Vec<PathBuf>` returned from `on_loaded`, calls
`walker::scan` for each, batches the resulting Tasks, and returns
them. Handlers stay pure state transitions.

The "one level deep only" restriction is enforced by the
`prefetching_paths` check: a prefetch-triggered scan result never
has an empty `remove` — it was put there when its scan was
issued. So the cascade-prevention check in `on_loaded` fires for
it, producing empty targets. A user-initiated scan never has its
path in `prefetching_paths` (or, if it did, `on_toggled` removed
it before the new scan was issued), so its result produces
non-empty targets.

## Incremental search (v0.6+)

Search is a **view-layer filter**; it doesn't mutate the node
graph. Three pieces coordinate:

1. **`SearchState { query, query_lower, visible_paths: HashSet,
   match_count }`** in `src/directory_tree/search.rs`. Held at
   `DirectoryTree::search: Option<SearchState>`. `None` when
   search is inactive (the default).
2. **`recompute_search_visibility()`** walks the whole loaded
   tree, populates `visible_paths` with matches plus their
   ancestor chains, and increments `match_count` per direct
   match. Called on: `set_search_query`, `set_filter`, and every
   successful `on_loaded`. O(N_loaded) per call.
3. **`DirectoryTree::visible_rows()`** dispatches: no search →
   delegate to `TreeNode::visible_rows` (the v0.1–0.5 walker
   that respects `is_expanded`); search active → walk via
   `collect_search_visible`, which skips non-visible nodes and
   **always descends** into children regardless of `is_expanded`.
   Both keyboard nav and view rendering go through this wrapper.

The view-layer `render_node` in `src/directory_tree/view.rs`
takes an optional `search_visible: Option<&HashSet<PathBuf>>`
parameter. Present → the same "skip non-visible, ignore
`is_expanded`" rule applies there too, giving visually identical
output to the `visible_rows` walker.

Search is case-insensitive basename-substring match. The
lowercased query is cached on `SearchState` at construction time
(`query_lower`), so per-node match checks don't re-lowercase.
Haystack is the node's `path.file_name()` (lowercased on demand
inside `matches_query`, which is O(basename_len) per node per
recompute — negligible for realistic trees).

Design constraints the implementation settled:

- **Already-loaded nodes only.** Typing doesn't spawn I/O. Apps
  that need broad coverage combine search with
  `with_prefetch_limit(N)`.
- **Sees through collapsed-but-loaded subtrees.** The walker
  descends regardless of `is_expanded`. Ancestors of matches
  force-render even when collapsed.
- **Selection is orthogonal.** `selected_paths` is untouched by
  search; per-node `is_selected` flags are preserved even on
  hidden rows.
- **Setter, not event.** `set_search_query(..)` is a plain
  mutator, matching `set_filter(..)` style. No new
  `DirectoryTreeEvent` variant. Apps wire their own
  `text_input::on_input` handler.
- **Empty query clears.** Two-state machine (`None` / `Some(...)`)
  avoids a "searching-for-nothing" pseudo-state.
- **Click-during-search doesn't escape.** The widget stays
  narrowed. Documented limitation; a future opt-in escape mode is
  possible but not default.
