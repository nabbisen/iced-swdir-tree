# RFC 001 — Generic item tree (`ItemTree<T>`)

**Status.** Implemented (v0.8.0)  
**Tracks.** New widget type `ItemTree<T>` for in-memory,
always-loaded, key-stable trees. Companion to
`dioxus-swdir-tree` RFC 012.  
**Touches.** `src/item_tree/`, `src/lib.rs` (re-exports),
`rfcs/README.md`, `CHANGELOG.md`, `ROADMAP.md`,
`docs/design/feature-specs.md` (new section),
`docs/design/porting-to-dioxus.md` (updated checklist).

---

## Summary

Add a second widget, `ItemTree<T>`, that provides the same
keyboard navigation, multi-select, expand/collapse, search, and
icon-theme surface as `DirectoryTree` but operates on
caller-supplied, in-memory node data rather than a filesystem.
Nodes are identified by a stable `NodeId(u64)` key; the whole
tree is updated atomically via `set_tree(root: ItemNode<T>)`,
which diffs the new tree against the current one and preserves
expansion and selection state for any key that survives the
update.

`ItemTree<T>` never issues async scans, holds no generation
counter, and has no `ScanExecutor`. It is the framework-agnostic
navigation model extracted from `DirectoryTree` with the
filesystem-loading layer removed.

---

## Motivation

A downstream user of `dioxus-swdir-tree` (`layered`, a Dioxus
Markdown editor) wants the widget's interaction model for an
in-memory section outline. Their nodes have `NodeId(u64)` keys
and string labels; there is no disk operation, no lazy loading.

The `dioxus-swdir-tree` author correctly declined to add a
generic tree to their crate alone, because doing so without a
shared spec would diverge from the cross-framework oracle.
The right place to anchor the spec is here, in the upstream
repository that owns `docs/design/feature-specs.md`.

The immediate driver is `layered`, but the design generalises:
any application that has a tree of domain objects and wants
keyboard-navigable, multi-selectable, expandable rows benefits
from `ItemTree<T>` without taking on the filesystem semantics
of `DirectoryTree`.

---

## Design

### [D1] Node identity: `NodeId(u64)`

A `NodeId` is a caller-assigned opaque integer. The widget
never generates or interprets IDs — it only tests them for
equality.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);
```

`u64` is wide enough that sequential allocation never wraps in
practice, and its `Copy + Hash + Eq` derive set is minimal.

A type-alias `type NodeId = u64` was considered but rejected:
the newtype prevents accidental confusion with unrelated `u64`
values in the calling code.

A generic key type `K: Eq + Hash + Clone` was considered but
rejected for v0.8: a single concrete type avoids propagating
a type parameter everywhere, and no real use case has yet
appeared that requires a different key type. If one does,
`NodeId` can be replaced with a generic parameter in a later
minor.

### [D2] Input shape: `ItemNode<T>`

```rust
pub struct ItemNode<T> {
    pub id: NodeId,
    pub data: T,
    pub children: Vec<ItemNode<T>>,
}
```

`children` encodes the hierarchy directly; there are no
parent-ID references. A node with `children.is_empty()` is a
leaf and never shows a caret.

An explicit `is_expandable: bool` flag was considered (to let
callers signal that a node *could* have children even if none
are provided yet). Rejected for v0.8: the primary use case
(document outline) always has complete data. Callers that want
a "loading" leaf can insert a placeholder child; if demand
for true lazy-loading emerges, it can be specified separately.

### [D3] Widget state: `ItemTree<T>`

```rust
pub struct ItemTree<T> {
    root: Option<ItemNodeState<T>>,   // None until first set_tree call
    selected_ids: Vec<NodeId>,
    active_id:    Option<NodeId>,
    anchor_id:    Option<NodeId>,
    search:       Option<ItemSearchState>,
    icon_theme:   Arc<dyn IconTheme>,
}
```

`ItemNodeState<T>` is the internal representation, distinct
from the caller-facing `ItemNode<T>`:

```rust
struct ItemNodeState<T> {
    id:          NodeId,
    data:        T,
    children:    Vec<ItemNodeState<T>>,
    is_expanded: bool,     // preserved across set_tree calls
    is_selected: bool,     // view hint; derived from selected_ids
}
```

There is no `is_loaded` field — all nodes are always loaded.
There is no `generation` counter — `ItemTree` issues no async
work.

### [D4] Atomic update with key-based diffing: `set_tree`

```rust
pub fn set_tree(&mut self, root: ItemNode<T>)
```

When the calling application rebuilds its data model (e.g.
after a document edit), it calls `set_tree` with the new root.
The widget diffs the new tree against the current tree using
`NodeId` as the key, preserving expansion and selection state
for surviving keys and discarding state for disappeared keys.

**Diffing algorithm:**

1. Walk the current tree, build `old_state: HashMap<NodeId,
   (is_expanded, is_selected)>`.
2. Walk the new `ItemNode<T>` tree, building `ItemNodeState<T>`:
   - If `node.id ∈ old_state`: copy `is_expanded`; set
     `is_selected` from `selected_ids`.
   - If `node.id ∉ old_state`: `is_expanded = false`,
     `is_selected = false`.
3. After rebuilding, update `selected_ids`: remove any id
   that no longer appears anywhere in the new tree.
4. Clear `active_id` and `anchor_id` if their ids have
   disappeared.

Key insight from the `layered` request: this model preserves
the user's interaction state through document edits without
requiring a full-reset API. A section the user opened stays
open; a section that was deleted disappears cleanly.

**Position changes:** a `NodeId` that moves from one parent
to another in the new tree retains its expansion state. Identity
(id) and position (parent–child relationship) are treated as
orthogonal — moving a node doesn't reset its state.

A `replace_rows` (full reset on every call) was explicitly
rejected at the request of the `layered` author. Their use
case (Markdown section outline rebuilt on every keystroke)
would lose all interaction state on each edit under that model.

### [D5] No async, no executor

`ItemTree<T>` issues no `Task` and holds no `Arc<dyn
ScanExecutor>`. `update()` always returns `Task::none()`.
There is no `Loaded` event variant.

```rust
pub enum ItemTreeEvent {
    Toggled(NodeId),
    Selected(NodeId, SelectionMode),
}
```

### [D6] Selection: same contract as `DirectoryTree`

- `selected_ids: Vec<NodeId>` — authoritative.
- `SelectionMode::{Replace, Toggle, ExtendRange}` — identical
  semantics; `ExtendRange` uses `visible_rows()`.
- Selection survives `set_tree` for surviving keys, drops for
  disappeared keys (silently; no event).
- `active_id` and `anchor_id` follow the same rules as
  `active_path` and `anchor_path` in `DirectoryTree`.

### [D7] Keyboard navigation: same contract as `DirectoryTree`

`handle_key(key: &Key, mods: Modifiers) -> Option<ItemTreeEvent>`

Identical bindings (↑↓ Home End Enter Space ← → Escape) with
`NodeId` substituted for `PathBuf`. The `Escape` key is only
bound during drag (deferred to v0.8.x).

### [D8] Search: matched against `T`'s display string

```rust
pub fn set_search_query(&mut self, query: impl Into<String>)
where T: Display
```

The match runs against `format!("{}", node.data).to_lowercase()`.
This is a full-string match (not basename-only as in
`DirectoryTree`), which is appropriate for labelled nodes.
All other search semantics are identical to `DirectoryTree`:
visible set = matches ∪ ancestors-of-matches; sees through
collapsed nodes; no I/O; selection survives.

`T: Display` is required only when `set_search_query` is
called — not on the struct itself. Apps that don't use search
can use any `T`.

### [D9] Icon themes: caret roles only

`ItemTree<T>` uses the installed `IconTheme` for
`CaretRight` and `CaretDown`. It does not use
`FolderClosed`, `FolderOpen`, `File`, or `Error` — those are
filesystem-specific roles.

A per-node icon hook (e.g. `with_icon_fn(Fn(&T) -> Option<IconSpec>)`)
was considered and deferred to v0.8.x. Callers that want icons
in rows can embed the glyph in their `T`'s `Display` output
or wait for the hook.

### [D10] Drag-and-drop: out of scope for v0.8

Drag-and-drop between nodes of an item tree is a meaningful
feature (reordering sections in `layered`). It is deferred
because the descendant-validity check used by `DirectoryTree`
relies on path-prefix comparison, which does not exist for
`NodeId`. A correct check requires explicit tree traversal, and
the interaction design (e.g. what "drop between" means vs "drop
into") needs more input from real use cases. Target: v0.8.x.

---

## Type bounds on `T`

`ItemTree<T>` requires `T: Clone + Debug + Send + Sync + 'static`.

- `Clone` — `set_tree` builds an internal `ItemNodeState<T>`
  by cloning data from the caller's `ItemNode<T>`.
- `Debug` — `ItemTree` derives `Debug`.
- `Send + Sync + 'static` — the widget is held in app state
  that may be moved across threads (e.g. in Dioxus signals).

`Display` is required additionally on call sites that invoke
`set_search_query`.

---

## Relationship to `DirectoryTree`

`ItemTree<T>` and `DirectoryTree` are sibling types, not
super/sub types. They share public API naming conventions
(builder pattern, same accessor names, same `SelectionMode`,
same `IconTheme`) but have separate implementations in v0.8.

A shared internal `nav_core` module — extracting the common
navigation state machine (selection, keyboard, visible_rows,
search) — is the natural next step once both implementations
are stable. That extraction is a v0.9 / `swdir-tree-core`
question, not a v0.8 question. The two parallel implementations
in v0.8 will serve as the empirical test of which code is truly
common and where the seams are.

---

## What this RFC is not

- **Not a `swdir-tree-core` extraction.** That is a separate,
  later decision.
- **Not a change to `DirectoryTree`.** The existing widget is
  unmodified.
- **Not a promise to support arbitrary data sources in
  `DirectoryTree`.** That widget stays filesystem-only.

---

## Open questions

**OQ1.** Should `selected_ids` be a `Vec` (ordered,
insertion-stable) or a `HashSet` (O(1) lookup)? The directory
tree uses `Vec` for `selected_paths` to preserve insertion
order and enable deterministic `ExtendRange`. Same choice here
seems right; confirm during implementation.

**OQ2.** Should `ItemTreeEvent::Selected` carry `bool is_dir`
like `DirectoryTreeEvent::Selected`? For a generic tree there
is no "is directory" concept — the flag would always be
`is_expandable`. Tentative: drop the bool; the app knows its
own data.

**OQ3.** Should `set_tree` accept only a single root node, or
a flat list of top-level nodes (`Vec<ItemNode<T>>`)? A single
root matches `DirectoryTree` (the root path is always present).
A flat list would be more natural for a section outline where
the "root" is the document itself (invisible). Tentative: single
root, with the app providing an invisible root node if needed.
Revisit if `layered` finds this awkward.

---

## Acceptance criteria

The RFC is considered implemented when:

1. `ItemTree<T>` compiles with `T: Clone + Debug + Send + Sync
   + 'static`.
2. `set_tree` correctly preserves expansion and selection for
   surviving keys and drops state for disappeared keys, verified
   by unit tests against hand-built trees.
3. Keyboard navigation, multi-select, and search pass all
   applicable clauses in `docs/design/feature-specs.md` with
   `NodeId` substituted for `PathBuf`.
4. `cargo test --all-features` passes.
5. The companion `docs/design/feature-specs.md` section for
   "Generic item tree" is written and committed alongside the
   implementation.
6. `dioxus-swdir-tree` RFC 012 has been cross-referenced with
   this RFC number.
