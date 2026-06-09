# Feature specifications

Precise behavioural specification for all ten features. Each
section is self-contained and reads as a test oracle — given
this input, produce this output. References to state fields use
the names from [data-model.md](data-model.md).

---

## Feature 1 — Lazy loading

**What it does.** Displays the directory tree one level at a
time. Only the root node is in memory at widget construction.
Children load when the user expands a folder.

### Specification

**S1.1 Root is always present.**  
After `DirectoryTree::new(root)`, the tree contains exactly one
node: the root. `root.path == root_path`, `root.is_dir == true`,
`root.is_loaded == false`, `root.is_expanded == false`,
`root.children == []`.

**S1.2 Expanding an unloaded folder issues exactly one scan.**  
When `Toggled(path)` is received for a folder where
`is_loaded == false`, the generation counter must be bumped
exactly once and a scan for `path` must be issued. If the
same folder is toggled again before the scan completes (a
second expand after collapse), a new scan is issued with a
fresh generation, and the first result will be discarded.

**S1.3 Scan results that don't match the current generation
are silently dropped.**  
If `payload.generation != tree.generation`, the tree state
must be identical before and after processing the payload.
No fields change, no side effects are emitted.

**S1.4 Once loaded, a folder is never re-scanned on
collapse/re-expand.**  
After `is_loaded = true`, a subsequent `Toggled` only flips
`is_expanded`. No scan is issued.

**S1.5 Loading depth is bounded by `max_depth`.**  
If `depth_of(root_path, path) > config.max_depth`, then when
`Toggled(path)` is received: set `is_loaded = true`,
`children = []`, issue no scan.

**S1.6 Error nodes are treated as loaded.**  
If a scan returns an error, the node gets `is_loaded = true`,
`error = Some(e)`, `children = []`. A subsequent `Toggled` on
that node triggers a retry scan — this is the only case where
an already-`is_loaded` node gets re-scanned. (Optional: a port
may choose to retry on every expand, or never retry without
explicit application action. The reference implementation does
not retry automatically; `is_loaded` stays true after an
error.)

---

## Feature 2 — Display filters

**What it does.** Three modes control which entries are shown:
`FoldersOnly`, `FilesAndFolders` (default), `AllIncludingHidden`.

"Hidden" is defined per OS: on Unix, a basename starting with
`.`; on Windows, the `FILE_ATTRIBUTE_HIDDEN` attribute plus the
dotfile rule as a fallback; on other platforms, the dotfile
rule only.

### Specification

**S2.1 Filter applies to children, not the root.**  
The root node is always visible regardless of filter.

**S2.2 `FoldersOnly` hides all files and all hidden
directories.**  
A hidden directory — one whose basename begins with `.` — is
also hidden under `FoldersOnly`.

**S2.3 `FilesAndFolders` hides hidden entries.**  
Files and directories whose basename begins with `.` are not
shown. Non-hidden files and non-hidden directories are shown.

**S2.4 `AllIncludingHidden` shows everything.**  
All entries returned by `scan_dir` are shown.

**S2.5 `set_filter` is instant and requires no re-scan.**  
The cache stores the raw, unfiltered entries. Changing the
filter re-derives all child lists from the cache. No async
work is issued.

**S2.6 Selection survives a filter change.**  
`selected_paths` is unchanged after `set_filter`. Per-node
flags are re-synced. A selected path that is hidden by the new
filter stays in `selected_paths` — it simply has no node
currently visible in the tree.

**S2.7 Expansion state survives a filter change.**  
Nodes that were `is_expanded = true` remain so. If a node's
children are rebuilt by the filter change, they inherit their
expansion state from the old node graph (matched by path).

---

## Feature 3 — Single-path selection

**What it does.** Tracks which path the user most recently
selected. Exposed via `selected_path()` (most recent) and
`is_selected(path)`.

### Specification

**S3.1 Initial state: nothing selected.**  
`selected_paths == []`, `active_path == None`.

**S3.2 `SelectionMode::Replace` sets exactly one path.**  
After a Replace selection: `selected_paths == [path]`,
`active_path == Some(path)`, `anchor_path == Some(path)`.

**S3.3 `selected_path()` returns `active_path`.**  
The v0.2 single-select accessor is a view onto `active_path`,
not the last element of `selected_paths`.

**S3.4 Clicking a selected row with Replace deselects others
but re-selects that row.**  
It does not toggle; it always results in exactly that one row
selected.

---

## Feature 4 — Keyboard navigation

**What it does.** `handle_key(key, modifiers)` translates a
keyboard event into a `DirectoryTreeEvent`, or returns `None`
if the key is not bound.

### Specification

All navigation uses `visible_rows()` to determine the
"current row" (the row for `active_path`). If `active_path`
is not in `visible_rows()`, navigation behaves as if there
is no current row (some keys may no-op).

**S4.1 `↑` / `↓` — move one row.**  
Produces `Selected(prev/next_path, is_dir, Replace)`.
At the top or bottom, the action is a no-op (does not wrap).

**S4.2 `Shift + ↑` / `Shift + ↓` — extend range.**  
Produces `Selected(prev/next_path, is_dir, ExtendRange)`.

**S4.3 `Home` / `End` — jump to first/last row.**  
Produces `Selected(first/last_path, is_dir, Replace)`.

**S4.4 `Shift + Home` / `Shift + End` — extend to first/last.**  
Produces `Selected(first/last_path, is_dir, ExtendRange)`.

**S4.5 `Enter` — toggle selected folder.**  
If `active_path` is a directory: produces `Toggled(path)`.
If `active_path` is a file or `None`: no-op.

**S4.6 `Space` — toggle selection of active row.**  
Produces `Selected(path, is_dir, Toggle)`.

**S4.7 `Ctrl + Space` — same as `Space`.**  
Some platforms use Ctrl+Space; the widget accepts both.

**S4.8 `←` — collapse or move to parent.**  
If `active_path` is an expanded directory: produces
`Toggled(path)` (collapse).
If `active_path` is a collapsed directory or a file: produces
`Selected(parent_path, true, Replace)`.
If at root: no-op.

**S4.9 `→` — expand or move to first child.**  
If `active_path` is a collapsed directory: produces
`Toggled(path)` (expand).
If `active_path` is an expanded directory: produces
`Selected(first_child_path, is_dir, Replace)`.
If `active_path` is a file: no-op.

**S4.10 `Escape` — cancel drag if active; unbound otherwise.**  
If drag is active: produces the event that cancels the drag.
If no drag: returns `None`. This is intentional — applications
that bind Escape for their own UI can do so safely when no drag
is in progress.

---

## Feature 5 — Pluggable scan executor

**What it does.** The `ScanExecutor` trait decouples
"how to run a blocking scan" from the widget. The default
implementation (`ThreadExecutor`) spawns a new OS thread per
scan. Custom implementations can use a tokio/smol/rayon pool.

### Specification

**S5.1 `ScanExecutor` is object-safe.**  
The trait must be implementable behind `Arc<dyn ScanExecutor>`.

**S5.2 The widget calls `spawn_blocking` once per scan.**  
Each call to `spawn_blocking` receives a `ScanJob` (a
`Box<dyn FnOnce() -> Vec<LoadedEntry>>`) and returns a
`ScanFuture` (a `Pin<Box<dyn Future<Output = Vec<LoadedEntry>>>>`).
The widget drives the future to completion via the framework's
async machinery.

**S5.3 The default executor is `ThreadExecutor`.**  
A tree built without `.with_executor()` uses `ThreadExecutor`,
which calls `std::thread::spawn` and returns a future that
resolves when the thread joins. This is correct but uses one
OS thread per expansion. High-throughput apps (many concurrent
expansions, or prefetch enabled) should plug in a bounded pool.

---

## Feature 6 — Multi-select

**What it does.** The `SelectionMode` enum enables Ctrl-toggle
and Shift-range selection. The selection set is a `Vec<PathBuf>`
and supports arbitrary cardinality.

### Specification

**S6.1 `SelectionMode::Toggle` adds if absent, removes if
present.**  
After Toggle(path): if path ∈ selected_paths, it is removed;
else it is appended. `anchor_path` is updated to path.
`active_path` is updated to path.

**S6.2 `SelectionMode::ExtendRange` replaces the set with
the range.**  
The range is determined by: `anchor_path` (fixed) and `path`
(the clicked row). All paths in `visible_rows()` between those
two indices (inclusive, whichever comes first) become the new
`selected_paths`. Paths outside the range are deselected.
If `anchor_path` is None, behaves as Replace.

**S6.3 The `anchor_path` does not change on `ExtendRange`.**  
Repeated Shift-clicks extend from the same anchor, allowing
the user to grow/shrink the range without losing the anchor.

**S6.4 Selection survives filter change and subtree reload.**  
`selected_paths` is not modified by `set_filter` or
`on_loaded`. Per-node flags are re-derived.

**S6.5 Hidden paths remain selected.**  
A path that is not currently in the rendered tree (filtered
out, unloaded parent, or excluded by search) may still be in
`selected_paths`. It becomes visible again when the condition
that hides it is removed.

---

## Feature 7 — Drag-and-drop

**What it does.** Tracks mouse press, drag, hover, and release.
Emits `DragCompleted { sources, destination }` when the user
drops on a valid target. The widget performs no filesystem
operations.

### Specification

**S7.1 Drag is activated by mouse-press on a row.**  
`Drag::Pressed(path, is_dir)` activates the drag. Sources are
set as described in the state machine.

**S7.2 Dropping on the same row as the press is a click.**  
If `Drag::Released(path)` and `path == started_at`, the drag
is cancelled and a deferred `Selected(path, is_dir, Replace)`
is emitted. This is how single-click selection is handled
without an explicit `onClick` event.

**S7.3 A valid drop target must be: a directory, not a source,
not a descendant of a source.**  
A path `D` is a valid target iff:
- `D` is a directory in the current node graph.
- `D ∉ drag.sources`.
- ∀ source S: `D` does not start with `S` (D is not a
  descendant of any source).

**S7.4 `Escape` cancels the drag.**  
Only bound when a drag is active. Returns `None` otherwise,
so applications can use Escape freely when no drag is in
progress.

**S7.5 The application performs all filesystem operations.**  
On `DragCompleted { sources, destination }`, the application
is expected to move/copy/rename/upload/whatever the sources.
The widget does not refresh automatically afterward — the app
must toggle the affected folders to trigger re-scans.

**S7.6 Drag sources persist during filter change or search.**  
`set_filter` and `set_search_query` do not clear `drag`. The
drag continues over the new view.

---

## Feature 8 — Parallel pre-expansion (prefetch)

**What it does.** When `prefetch_per_parent > 0`, after a
user-initiated scan completes, the widget speculatively issues
background scans for up to `N` of the just-loaded folder's
direct folder-children.

### Specification

**S8.1 Prefetch is disabled by default.**  
`prefetch_per_parent = 0`. A tree built without
`with_prefetch_limit` behaves identically to v0.4.

**S8.2 Prefetch triggers exactly once per user-initiated scan.**  
When `on_loaded` merges a result for a path P that is NOT in
`prefetching_paths`, up to `prefetch_per_parent` folder-children
of P are selected for prefetch. Each selection bumps the
generation and issues a scan.

**S8.3 Prefetch does NOT cascade.**  
When `on_loaded` merges a result for a path P that IS in
`prefetching_paths`, P is removed from `prefetching_paths` and
no further prefetch is triggered. This prevents exponential
`per_parent ^ depth` fan-out.

**S8.4 Prefetch loads but does not expand.**  
Prefetch sets `is_loaded = true` on the child nodes. It does
NOT set `is_expanded`. The user must click to expand; when they
do, it is an instant fast-path (Case C of `Toggled`).

**S8.5 The prefetch skip list applies.**  
A folder-child whose `basename` matches any entry in
`config.prefetch_skip` (exact, ASCII case-insensitive) is
excluded from prefetch target selection. The skip list never
prevents user-initiated scans.

**Default skip list:**  
`.git`, `.hg`, `.svn`, `node_modules`, `__pycache__`, `.venv`,
`venv`, `target`, `build`, `dist`.

**S8.6 `max_depth` applies to prefetch.**  
A folder-child at depth > `max_depth` is excluded from prefetch
targets.

**S8.7 If the user clicks a prefetching path, the user wins.**  
When `Toggled(P)` fires while `P ∈ prefetching_paths`:
1. Remove P from `prefetching_paths`.
2. Bump generation, issue user-initiated scan.
3. The original prefetch scan result will arrive with a stale
   generation and be discarded.
4. The user-initiated result triggers its own prefetch wave.

---

## Feature 9 — Incremental search

**What it does.** `set_search_query(q)` activates a
live-filter that narrows visible rows to basename matches plus
their ancestor chain.

### Specification

**S9.1 Match semantics: basename substring, case-insensitive.**  
A path P matches query Q iff:
`basename(P).to_lowercase().contains(Q.to_lowercase())`.
Only the basename is matched — path components like `/src/`
do not match a query for `"src"`.

**S9.2 Visible set: matches ∪ ancestors-of-matches.**  
After activating a query, the visible set is:
```
visible = { P | P matches Q }
        ∪ { A | A is a proper ancestor of some P that matches Q }
```
A node is drawn iff its path is in this set.

**S9.3 Search sees through collapsed-but-loaded subtrees.**  
`recompute_search_visibility` walks the tree regardless of
`is_expanded`. A match inside a collapsed (but loaded) subtree
is visible in search mode; its ancestor chain is force-shown.

**S9.4 Empty string clears the search.**  
`set_search_query("")` ≡ `clear_search()`. There is no
"active search for the empty string" state.

**S9.5 Selection is orthogonal to search.**  
Activating or clearing search never modifies `selected_paths`.
A selected row hidden by search remains selected; it reappears
when the search clears.

**S9.6 Filter and search compose correctly.**  
Filter applies first (during `on_loaded` / `set_filter`).
Search then runs over the filter-surviving nodes. Changing
the filter while search is active re-runs
`recompute_search_visibility` over the new node graph.

**S9.7 Loading while search is active re-runs the search.**  
When `on_loaded` merges new children, `recompute_search_visibility`
is called. A match that was inside an unloaded subtree now
becomes visible.

**S9.8 `match_count` counts direct matches only.**  
Ancestor rows shown for context are NOT counted. Apps that
display "N results" should use `search_match_count()`, not
`visible_paths.len()`.

**S9.9 Search does not trigger I/O.**  
`set_search_query` issues no async tasks. It is purely
in-memory filtering over already-loaded nodes.

**Known limitation:** Clicking a folder during search does not
"escape" the filter. The visible set remains the search-computed
set regardless of expansion state. Clearing the search is the
documented way to switch to explore mode.

---

## Feature 10 — Icon themes

**What it does.** An `IconTheme` trait controls which glyph,
font, and size the view renders for each of the six logical
icon positions.

### Specification

**S10.1 The six icon roles are fixed (for v0.7):**
- `FolderClosed` — a collapsed directory.
- `FolderOpen` — an expanded directory.
- `File` — a non-directory entry.
- `Error` — a directory whose scan failed.
- `CaretRight` — collapsed indicator (caret pointing right).
- `CaretDown` — expanded indicator (caret pointing down).

**S10.2 `IconRole` is `#[non_exhaustive]`.**  
Future minor releases may add roles. External theme
implementations must include a `_ =>` fallback arm to
forward-proof their `match`.

**S10.3 `IconSpec` has three public fields:**
```
glyph: Cow<'static, str>   ← text to render
font:  Option<Font>         ← None = default font
size:  Option<f32>          ← None = widget default (14.0)
```

**S10.4 `IconTheme` is called during rendering only.**  
The theme is not consulted during state transitions. A port
may pass the theme directly to the render function without
threading it through the state.

**S10.5 The default theme is feature-dependent:**
- With `icons` feature: `LucideTheme` (lucide vector glyphs,
  requires the lucide TTF to be registered with the font stack).
- Without `icons` feature: `UnicodeTheme` (Unicode symbols in
  any system font; no font registration required).

**S10.6 `LucideTheme` requires font registration.**  
Without the lucide TTF registered with the rendering engine,
`LucideTheme`'s codepoints render as tofu. The widget still
functions; only the icons look wrong. The TTF bytes are exposed
as `LUCIDE_FONT_BYTES` for apps to register.

**S10.7 A custom theme need only implement one method:**
```
fn glyph(&self, role: IconRole) -> IconSpec
```
The method should be cheap and pure — build the mapping at
construction time if needed.

---

## Feature 11 — Generic item tree (`ItemTree<T>`)

**What it does.** Provides the same keyboard navigation,
multi-select, expand/collapse, and search as `DirectoryTree`,
for caller-supplied in-memory node data. No async I/O.

### Specification

**S11.1 Node identity is `NodeId(u64)` — opaque, caller-assigned,
unique within the tree.**
The widget uses it only for equality checks. Duplicate IDs
produce unspecified diffing behaviour.

**S11.2 `ItemNode<T>` is the caller-facing input type.**
`{ id: NodeId, data: T, children: Vec<ItemNode<T>> }`.
A node with `children.is_empty()` is a leaf (no caret rendered).

**S11.3 All nodes are always "loaded".**
There is no `is_loaded` flag and no async scan lifecycle.
`update()` always returns `Task::none()`.

**S11.4 `set_tree` replaces the tree with key-based diffing.**
For each NodeId in the new tree:
- If it existed in the old tree: copy `is_expanded` and
  `is_selected` state.
- If it is new: start collapsed and unselected.
For each NodeId that disappears: silently remove from
`selected_ids`, `active_id`, and `anchor_id`.

**S11.5 Position changes preserve state.**
A NodeId that moves from one parent to another in the new tree
retains its expansion and selection state.

**S11.6 Search matches against `format!("{}", node.data)`.**
Full-string lowercase substring match (not basename-only as in
`DirectoryTree`). Requires `T: Display`. All other search
semantics (visible = matches ∪ ancestors, sees through collapse,
selection orthogonal) are identical to S9.

**S11.7 Keyboard, multi-select, icon themes: identical spec
to `DirectoryTree` with `NodeId` substituted for `PathBuf`.**

**S11.8 `SelectionMode::ExtendRange` uses `visible_rows()`.**
Same algorithm as `DirectoryTree` S6.2, over the
`ItemTree::visible_rows()` list.

**S11.9 Drag-and-drop: opt-in via `with_drag_and_drop(true)`.**
Off by default. When disabled, v0.8 behaviour is preserved
byte-identical: a press emits `Selected(_, Replace)` directly.

**S11.10 Drag is activated by mouse-press on a row body.**
`ItemDragMsg::Pressed(id)` starts a drag. Sources are the full
`selected_ids` set (in tree order) if `id` is selected, otherwise
`[id]` alone.

**S11.11 Dropping on the same node as the press is a click.**
`ItemDragMsg::Released(id, _)` with `id == pressed_id` cancels
the drag and emits a deferred `Selected(id, Replace)`. Selection
is never mutated directly on press.

**S11.12 A valid drop target is determined by four rules.**
A drop of sources `S` at `(target, position)` is valid iff:
(1) `target` is a live node;
(2) `target ∉ S`;
(3) for `Before`/`After`, `target` is not the root (no sibling slot);
(4) the effective new parent (`target` for `Into`, else `target`'s parent)
is not any `s ∈ S` nor a descendant of any `s`.

**S11.13 `Escape` cancels the drag.**
Only bound while a drag is active; returns `None` otherwise.

**S11.14 The application performs all model mutations.**
On `DragCompleted { sources, target, position }`, the application
moves the nodes in its own data model, rebuilds the `ItemNode<T>`
tree, and calls `set_tree`. The widget never reorders its own
internal tree directly.

**S11.15 `DropPosition` semantics.**
`Before` — insert sources as siblings immediately before `target`.
`Into` — append sources as the last children of `target`.
`After` — insert sources as siblings immediately after `target`.

**S11.16 Drag state is preserved through `set_search_query`.**
An active drag continues over the filtered view.
