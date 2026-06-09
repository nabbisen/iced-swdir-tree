# RFC 002 — Drag-and-drop for `ItemTree<T>`

**Status.** Implemented (v0.9.0)  
**Tracks.** Drag-and-drop reorder/nest support for the in-memory
`ItemTree<T>` widget introduced in RFC 001. Resolves the [D10]
deferral in RFC 001 and the `Escape`-during-drag note in RFC 001
[D7] for `ItemTree`.  
**Touches.** `src/item_tree.rs`, `src/item_tree/drag.rs` (new),
`src/item_tree/node.rs` (parent-map helper), `src/lib.rs`
(re-exports), `examples/item_tree.rs`, `tests/item_tree_drag_drop.rs`
(new), `docs/design/feature-specs.md` (Feature 11 drag clauses),
`docs/guide/drag-and-drop.md`, `CHANGELOG.md`, `ROADMAP.md`,
`rfcs/README.md`.

---

## Summary

Add opt-in drag-and-drop to `ItemTree<T>`, enabling the user to
**reorder** nodes among their siblings and **nest** a node under
another. The widget tracks the gesture through a small state
machine — mirroring the one `DirectoryTree` has carried since
v0.4 — and emits a single
`ItemTreeEvent::DragCompleted { sources, target, position }`
event when the user releases over a valid drop target. As with
`DirectoryTree`, the widget mutates **nothing**: the application
performs the move in its own data model and feeds the new tree
back through `set_tree`, whose key-based diffing (RFC 001 [D4])
preserves expansion and selection across the edit.

Drag-and-drop is **off by default**. Existing `ItemTree`
behaviour is byte-identical unless `with_drag_and_drop(true)` is
called.

---

## Motivation

RFC 001 [D10] deferred drag-and-drop for two stated reasons:

1. The descendant-validity check `DirectoryTree` uses relies on
   `PathBuf::starts_with` (component-wise, O(1)); `NodeId` has no
   such relation and a correct check needs explicit tree
   traversal.
2. The interaction design — "what does *drop between* mean vs
   *drop into*?" — needed more input from real use cases.

Both have resolved into ordinary design work rather than
blockers:

- **The traversal cost is bounded and cheap.** `ItemTree` holds
  small, fully-in-memory trees (the driving case is a document
  outline). The validity check is made O(depth)-per-hover by
  snapshotting a child→parent map once when the drag begins (see
  [D6]); the snapshot itself is O(n) and runs on a single mouse
  press, not on every cursor move.
- **The use case supplies the interaction.** `layered` (the
  Dioxus Markdown editor that drove RFC 001) wants to reorder
  sections — that is *drop between siblings* — and to nest a
  section under another — *drop into*. Both, together, define a
  clean three-position drop model ([D3]). The ambiguity RFC 001
  flagged is removed by making the target node **and** the drop
  position explicit in the event, rather than inferring intent
  from cursor geometry.

`DirectoryTree`'s drag machinery (state machine, deferred
selection, `Escape`-to-cancel, hover highlight, source-set rule)
is a tested template; most of this RFC is the careful adaptation
of that template from `PathBuf`-into-folder semantics to
`NodeId`-reorder/nest semantics.

---

## Non-goals

- **Not a `tree-nav-core` / `swdir-tree-core` extraction.** That
  remains deferred (see [D11]). This RFC keeps the two widgets as
  independent siblings.
- **Not a change to `DirectoryTree`.** Its existing
  drop-into-folder drag model is untouched.
- **Not a generic "move my data for me" API.** The widget reports
  intent; the application owns the mutation, exactly as
  `DirectoryTree` does for filesystem moves ([D9]).
- **Not horizontal-cursor "outdent on drop" resolution.** The
  Workflowy-style "the gap means a different parent depending on
  where you aim horizontally" behaviour is out of scope; the drop
  position is always relative to one concrete target node ([D3],
  OQ1).

---

## Design

### [D1] Scope of this advancement

Of the two subjects deferred for consideration alongside the
`dioxus-swdir-tree` work — `ItemTree` drag-and-drop and the
`tree-nav-core` extraction — this RFC implements **only** the
former. The extraction's gating precondition (a Dioxus
implementation to compare against) is not yet met; see [D11].

### [D2] Opt-in via builder, default off

```rust
pub fn with_drag_and_drop(mut self, enabled: bool) -> Self
```

When disabled (the default), the view renders rows exactly as in
v0.8 and emits `Selected(_, Replace)` on press — behaviour is
unchanged and there is no drag instrumentation. When enabled, the
view switches to the press / enter / exit / release model and
renders inter-row drop zones.

`DirectoryTree` wires drag unconditionally, but a filesystem tree
is inherently rearrangeable whereas many item trees are
read-only navigation surfaces (a settings outline, a static menu).
Gating keeps those cases free of the deferred-selection click
model and the drop-zone chrome. This matches the crate's existing
opt-in posture for non-universal behaviour (e.g. `with_prefetch_limit`).

### [D3] Drop model: `DropPosition`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPosition {
    /// Insert the source(s) as sibling(s) immediately *before*
    /// `target`, under `target`'s parent.
    Before,
    /// Insert the source(s) as the *last child/children* of
    /// `target` (nest).
    Into,
    /// Insert the source(s) as sibling(s) immediately *after*
    /// `target`, under `target`'s parent.
    After,
}
```

Each position has an unambiguous `(new_parent, insertion_point)`
meaning that does **not** depend on which visible row happens to
be adjacent:

| Position | New parent        | Insertion point                    |
| -------- | ----------------- | ---------------------------------- |
| `Before` | parent of target  | just before `target` in that list  |
| `Into`   | `target` itself   | end of `target`'s children         |
| `After`  | parent of target  | just after `target` in that list   |

`DropPosition` is deliberately **exhaustive** (not
`#[non_exhaustive]`): a drop relative to a node is before it,
into it, or after it — the set is closed. Apps are expected to
handle all three.

### [D4] Event shape

Two variants are added to `ItemTreeEvent`:

```rust
pub enum ItemTreeEvent {
    Toggled(NodeId),
    Selected(NodeId, SelectionMode),
    /// Opaque internal drag machinery — route back to `update`.
    Drag(ItemDragMsg),
    /// User completed a drag with intent to place `sources` at
    /// `position` relative to `target`.
    DragCompleted {
        sources: Vec<NodeId>,
        target: NodeId,
        position: DropPosition,
    },
}
```

This mirrors `DirectoryTreeEvent::{Drag, DragCompleted}` but
replaces the single `destination: PathBuf` with the richer
`target: NodeId` + `position: DropPosition` pair — the "distinct
event shape" RFC 001 [D10] anticipated.

`ItemTreeEvent` is **not** marked `#[non_exhaustive]`, matching
its sibling `DirectoryTreeEvent`, which has also grown variants
across minor releases without that attribute. Adding variants in
a `0.x` minor is permitted by SemVer's `0.x` rules, and the
parallel between the two event enums is worth more to readers
than the marginal forward-compat the attribute would buy.

`ItemDragMsg` is opaque, like `DragMsg`: applications route it
back through `update` unchanged and never construct it by hand.

```rust
#[derive(Debug, Clone)]
pub enum ItemDragMsg {
    Pressed(NodeId),
    Entered(NodeId, DropPosition),
    Exited(NodeId, DropPosition),
    Released(NodeId, DropPosition),
    Cancelled,
}
```

`Entered`/`Exited`/`Released` carry the `DropPosition` because in
`ItemTree` it is the **view** (which drop zone the cursor is
over) that determines position — unlike `DirectoryTree`, where a
drop is always "into" the hovered row.

### [D5] Source set at drag start

Identical rule to `DirectoryTree`: on `Pressed(id)`, if `id` is
in the current selection, the drag carries the whole
`selected_ids` set; otherwise it carries `[id]` alone. Pressing
an unselected row drags only that row. `sources` is reported in
**tree (document) order**, not selection order (OQ2).

### [D6] Validity via a parent-map snapshot

When a drag begins (`Pressed`), the widget snapshots into
`ItemDragState`:

- `sources: Vec<NodeId>` — per [D5].
- `parent: HashMap<NodeId, Option<NodeId>>` — child→parent for
  every live node (`None` for the root). Built by one O(n) walk.
- `primary: NodeId` — the pressed node, for the click branch.

A drop of `sources = S` at `(target, position)` is **valid** iff:

1. `target` is a live node.
2. **Effective parent exists where required.** The *effective
   new parent* is `target` for `Into`, and `parent[target]` for
   `Before`/`After`. For `Before`/`After`, if `target` is the
   root (`parent[target] == None`), the drop is invalid — the
   root has no sibling slot.
3. `target ∉ S` for `Before`/`After` (placing a node relative to
   one of the moving nodes is undefined); for `Into`,
   `target ∉ S` as well (can't nest a node into itself).
4. **No cycle.** For every `s ∈ S`, the effective new parent must
   not be `s` and must not be a descendant of `s`. Equivalently:
   `s` must not lie on the ancestor chain of the effective parent
   (inclusive). The chain is walked through the snapshotted
   `parent` map — O(depth) per source, no live-tree access.

Rule 4 is the `NodeId` analogue of `DirectoryTree`'s
`!target.starts_with(source)` rule; the parent-map walk is what
replaces the free O(1) path-prefix test. Because the map is
snapshotted at press time, every subsequent `Entered` hover is a
pure O(depth) lookup with no borrow of the live tree.

Validity is recomputed on each `Entered`; the view reads the
current `(target, position)` hover to paint a drop indicator. An
invalid hover clears the indicator (no drop will fire there).

### [D7] Deferred selection (click vs drag)

Mirrors `DirectoryTree` S7.2. `Pressed` does **not** change the
selection. If the gesture ends with `Released(id, Into)` on the
**same** node that was pressed and no other row was crossed (the
hover never became a different valid target), it was a *click*:
the widget emits a deferred `Selected(id, SelectionMode::Replace)`.
This preserves multi-selection through a click-without-drag,
exactly as the directory widget does.

### [D8] `Escape` cancels an in-flight drag

`handle_key` binds `Escape` **only while a drag is active**,
emitting `ItemDragMsg::Cancelled`; otherwise it returns `None` so
applications keep `Escape` for their own use. This also closes
the `Escape`-during-drag item RFC 001 [D7] left open for
`ItemTree`.

### [D9] The widget mutates nothing

On `DragCompleted`, the application moves the `sources` to the
requested position in its own model, rebuilds its `ItemNode<T>`
tree, and calls `set_tree` (or `set_tree_and_recompute_search`).
RFC 001 [D4] diffing then preserves expansion/selection for every
surviving `NodeId` — including the moved nodes, since identity is
orthogonal to position (RFC 001 [D4] "Position changes"). The
widget never reorders its own internal tree directly; the only
path that mutates structure is `set_tree`. This keeps the single
source of truth (the app's model) authoritative and mirrors
`DirectoryTree` S7.5.

`DragCompleted` guarantees: `sources` is non-empty; `target` is
live; the `(target, position)` pair passed the [D6] validity
rules. The widget reports order in tree order.

### [D10] Version: v0.9.0

This adds public API (`DropPosition`, `ItemDragMsg`, two
`ItemTreeEvent` variants, one builder) and is therefore **feature
work**, which the project's release discipline routes to a
**minor** bump. RFC 001's informal "Target: v0.8.x" wording is
superseded here: a patch (`0.8.z`) is reserved for safety valves
and refactors, never for new surface. v0.9.0 also pushes the
planned v1.0 API-freeze out by exactly this surface, which is the
correct outcome — freezing before reorder support landed would
have shipped an incomplete `ItemTree`.

### [D11] `tree-nav-core` extraction stays deferred

Recorded here for traceability: the shared-navigation-core
extraction (RFC 001 "Relationship to `DirectoryTree`") is **not**
undertaken in this RFC. Its stated precondition is the ability to
compare the iced and Dioxus implementations for convergence, and
`dioxus-swdir-tree` does not yet exist. Extracting now would fix
the module seams against single-framework evidence.

Landing drag-and-drop in `ItemTree` *improves* the eventual
extraction: drag is a navigation concern, and having it in both
`DirectoryTree` and `ItemTree` makes the truly-common surface
(selection, keyboard, `visible_rows`, search, **and now drag
state**) visible before any line is moved into a core crate. The
extraction should be a separate RFC opened once the Dioxus port
provides the second data point.

---

## State machine

```
                         Pressed(id)
   Idle ───────────────────────────────────────────▶ Dragging
    ▲                                                   │
    │  Released(id, Into) on same id, no cross  ┌───────┤
    │  → emit Selected(id, Replace)             │       │ Entered(t, pos) → revalidate hover
    │                                           │       │ Exited(t, pos)  → clear hover if it was (t,pos)
    │  Released(t, pos), hover valid            │       │
    │  → emit DragCompleted{sources,target,pos} │       │
    │                                           │       │
    │  Released elsewhere / Cancelled / Escape  │       │
    │  → drop to Idle (selection untouched)     │       │
    └───────────────────────────────────────────┘◀──────┘
```

`Released` and `Cancelled` are idempotent; a stray `Entered`
without a prior `Pressed` is a no-op (matches `DirectoryTree`).

---

## Type bounds

No new bounds. `with_drag_and_drop` and the state machine require
only the existing `T: Clone + Debug + Send + Sync + 'static`. The
drop-zone view path requires `T: Display`, same as the existing
`view`.

---

## Open questions

**OQ1.** Should `Before`/`After` eventually resolve via
visible-row adjacency plus horizontal cursor position (so a single
gap can target different ancestors at different indent levels, à
la Workflowy/Notion)? The current target-relative model is
unambiguous and sufficient for `layered`. Revisit if outline apps
ask for outdent-on-drop. Out of scope for v0.9.

**OQ2.** Multi-source drop ordering: the widget reports `sources`
in tree order. Is selection order ever wanted instead? Tentative:
no — tree order is deterministic and the app can re-sort. Confirm
with `layered`.

**OQ3.** Should drag-and-drop be opt-in ([D2]) or always-on like
`DirectoryTree`? Chose opt-in to preserve v0.8 behaviour and serve
read-only trees. Revisit if the split proves to be friction.

---

## Acceptance criteria

The RFC is implemented when:

1. `ItemTree::with_drag_and_drop(true)` enables drag; default and
   `false` leave v0.8 behaviour byte-identical.
2. `DropPosition`, `ItemDragMsg`, and the two new `ItemTreeEvent`
   variants are public and re-exported from the crate root.
3. The [D6] validity rules are covered by unit tests over
   hand-built `ItemDragState` snapshots: self-drop, descendant
   (cycle) drop, sibling reorder, nest, root-sibling rejection,
   and multi-source drops — without constructing a live widget.
4. The state machine (`Pressed`/`Entered`/`Exited`/`Released`/
   `Cancelled`) and the deferred-selection click branch ([D7])
   are covered by integration tests in
   `tests/item_tree_drag_drop.rs`.
5. `examples/item_tree.rs` enables drag-and-drop and performs an
   actual reorder/nest on `DragCompleted`, then `set_tree`s the
   result, demonstrating state preservation across the move.
6. `docs/design/feature-specs.md` Feature 11 gains numbered drag
   clauses (the test oracle) and S11.9 is updated from "out of
   scope" to the shipped behaviour.
7. `cargo fmt`, `cargo clippy --all-targets --all-features
   -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc`,
   `cargo test --all-features`, and `cargo publish --dry-run` all
   pass.
8. `dioxus-swdir-tree`'s checklist is informed that `ItemTree`
   drag-and-drop now has an upstream spec (cross-reference left
   for the port author; no change required here).
