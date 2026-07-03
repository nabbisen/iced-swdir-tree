# Handoff — `iced-swdir-tree` → `dioxus-swdir-tree`

**From.** `iced-swdir-tree` v0.9.0  
**To.** `dioxus-swdir-tree` / `dioxus-swdir-tree-core` maintainer  
**Date.** 2026-06-09  
**Covers.** Everything new in `iced-swdir-tree` since v0.7.2
(the last release before `dioxus-swdir-tree-core` was built
from these design documents). One architectural decision and one
concrete feature need to propagate to the Dioxus project.

---

## Summary

| Item | Status in iced | Status in dioxus | Action needed |
| --- | --- | --- | --- |
| `ItemTree<T>` — basic (RFC 001) | ✅ v0.8.0 | ✅ v0.8.0 | None — already implemented |
| `swdir` version | ✅ 0.11 (v0.9.0) | ✅ 0.11 | None — already aligned |
| ItemTree drag-and-drop (RFC 002) | ✅ v0.9.0 | ❌ deferred | **Implement** — full spec below |
| Share-design-not-code (RFC 003 withdrawn) | ✅ decided | Informational | Read § Architecture decision |

---

## 1. Architecture decision — share design, not code

`iced-swdir-tree` v0.9.0 opens and immediately withdraws RFC 003
("Extract a framework-agnostic `swdir-tree-core`"). The decision
is recorded in `rfcs/archive/003-extract-swdir-tree-core.md`.

**Resolved position:** the shared asset between the two projects
is the **design** (`docs/src/internals/`), not a shared crate. Each
project implements the spec using the data structures and async
model that fit its own architecture.

Why code-sharing was rejected:

1. **The logic is not representation-neutral.** The "pure"
   algorithms (drag validity, selection range, search) are written
   against each crate's data structure. `dioxus-swdir-tree-core`'s
   drag check walks parent links in its arena; `iced-swdir-tree`'s
   builds a parent-map snapshot from a nested tree. To share the
   code you would first have to impose one data structure on both —
   which is exactly the architecture-level choice that should remain
   free per framework.

2. **The async models are incompatible.** `iced-swdir-tree` uses
   `Task`/`ScanExecutor`; `dioxus-swdir-tree-core` returns
   `ScanRequest` as data to a coroutine. A shared core cannot be
   idiomatic for both.

3. **The design-docs model already works.** `dioxus-swdir-tree-core`
   was built "following the design documents of `iced-swdir-tree`
   v0.7" and produced a faithful, independently-tested implementation
   with no shared dependency. That is the proof.

`dioxus-swdir-tree-core`'s internal core/view split is the right
architectural call *for Dioxus*. There is no expectation that
`iced-swdir-tree` mirrors it.

**For `dioxus-swdir-tree`:** no action required. The Dioxus project
should continue treating `docs/src/internals/` as its upstream spec and
implementing each feature idiomatically in its own arena-based core.

---

## 2. `swdir` version alignment

`iced-swdir-tree` v0.9.0 bumps its `swdir` dependency from
`0.10` to `0.11`, aligning with `dioxus-swdir-tree-core 0.8.0`
which already uses `swdir 0.11`. Both projects are now on the
same minor. No API changes were required on the iced side.

**For `dioxus-swdir-tree`:** no action — already aligned.

---

## 3. ItemTree drag-and-drop (RFC 002) — the main handoff item

### 3.1 What was built

`iced-swdir-tree` v0.9.0 implements opt-in drag-and-drop
reorder/nest for `ItemTree<T>`. The implementation is fully tested
(27 new tests: 12 unit, 15 integration) and the spec is recorded
as clauses S11.9–S11.16 in `docs/src/internals/feature-specs.md` and as
transitions in `docs/src/internals/state-machine.md` (§ ItemTree drag
transitions). These documents are the oracle for the port.

The complete RFC 002 design rationale lives at
`rfcs/done/002-item-tree-drag-and-drop.md`.

### 3.2 Why it differs from DirectoryTree drag

`DirectoryTree` drag is "drop *into* a folder" — there is a single
`destination: PathBuf` and a single drop semantic (move sources
into the destination directory).

`ItemTree` drag adds two new semantics:

| `DropPosition` | Meaning |
| --- | --- |
| `Before` | Sources become siblings immediately *before* `target` (reorder) |
| `Into` | Sources become the last *children* of `target` (nest) |
| `After` | Sources become siblings immediately *after* `target` (reorder) |

Each position maps to an unambiguous `(new_parent, insertion_index)`
that does not depend on cursor geometry:

| Position | New parent | Insertion index |
| --- | --- | --- |
| `Before` | `parent_of(target)` | `index_of(target)` |
| `Into` | `target` | `target.children.len()` |
| `After` | `parent_of(target)` | `index_of(target) + 1` |

### 3.3 The event shape

```
ItemTreeEvent::Drag(ItemDragMsg)        -- opaque; route back to update
ItemTreeEvent::DragCompleted {
    sources:  Vec<NodeId>,   -- in tree pre-order
    target:   NodeId,
    position: DropPosition,
}
```

`ItemDragMsg` variants (route back to update unchanged):

```
Pressed(NodeId)                         -- start drag
Entered(NodeId, DropPosition)           -- cursor over drop zone
Exited(NodeId, DropPosition)            -- cursor left drop zone
Released(NodeId, DropPosition)          -- mouse up
Cancelled                               -- Escape or external abort
```

The `position` in `Entered`/`Exited`/`Released` is determined by
the **view** — which of the three per-row zones the cursor is in.
This differs from `DirectoryTree` where the view emits just the
node (the position is always "Into").

### 3.4 Validity rules (S11.12)

A drop of sources `S` at `(target, position)` is valid iff all
of the following hold:

1. `target` is a live node.
2. `target ∉ S`.
3. For `Before`/`After`: `target` is not the root (root has no
   sibling slot).
4. **No cycle:** the *effective new parent* (`target` for `Into`,
   otherwise `parent_of(target)`) is not any `s ∈ S`, and is not
   a descendant of any `s` (i.e., no source appears on its ancestor
   chain, inclusive).

**`dioxus-swdir-tree-core` implementation note:** the arena in
`dioxus-swdir-tree-core` stores parent links natively in
`InternalItem`. Rule 4 is therefore a simple ancestor chain walk:

```rust
fn is_ancestor_or_self(store: &HashMap<NodeId, InternalItem>, maybe_anc: NodeId, node: NodeId) -> bool {
    let mut cur = Some(node);
    while let Some(c) = cur {
        if c == maybe_anc { return true; }
        cur = store[&c].parent;
    }
    false
}
```

In `iced-swdir-tree` this check requires a parent-map *snapshot*
(because its nested-tree structure has no parent links). The
Dioxus arena makes the check directly against the live store —
**no snapshot needed**. This is an area where the arena model is
concretely simpler than iced's nested tree for DnD.

### 3.5 Deferred selection (S11.11)

`Pressed` does **not** change selection. If the gesture ends with
`Released(id, _)` on the **same `NodeId` as the press** and the
hover has not been set to a different valid target, it was a click.
Emit a deferred `Selected(id, Replace)`.

This prevents a click on a multi-selected row from collapsing the
selection before the drag could start — the same deferred-selection
pattern `DirectoryTree` uses.

### 3.6 Escape cancels (S11.13)

`handle_key` binds `Escape` to `Drag(Cancelled)` **only while a
drag is active**. When no drag is in progress, `Escape` returns
`None` so the host application can handle it freely.

### 3.7 Opt-in toggle (S11.9)

Drag-and-drop is **off by default**. Enable it with a builder:

```rust
let tree = ItemTree::new().with_drag_and_drop(true);
```

When disabled, the view emits `Selected(_, Replace)` on mouse
press (v0.8 behaviour, unchanged).

### 3.8 Widget responsibility (S11.14)

The widget mutates **nothing** on `DragCompleted`. The host
application:

1. Receives `DragCompleted { sources, target, position }`.
2. Computes the new tree from that intent (extract sources from
   the model, insert at the new position).
3. Calls `set_tree` (or `set_tree_and_recompute_search`).
4. Key-based diffing preserves expansion and selection for all
   surviving ids, *including the moved nodes* (identity is
   orthogonal to position in the tree).

A worked example of the full `apply_move` → `set_tree` cycle is
in `examples/item_tree.rs`.

### 3.9 View layer — three drop zones per row

The iced view renders **three zones per row**:

```
┌─────────────────────────────────────────────┐
│  [Before strip  — thin, ~6px, full-width  ] │  → Entered(id, Before)
│  [Row body  — caret + label, full height  ] │  → Pressed + Entered(id, Into)
│  [After strip  — thin, ~6px, full-width   ] │  → Entered(id, After)
└─────────────────────────────────────────────┘
```

Each strip/body is a separate mouse-target. The active zone
shows a highlight: Before/After zones show an insertion bar
(primary-palette fill); the body shows a success-palette outline
when it is the Into hover target.

**For the Dioxus port:** replace `mouse_area` with `onmouseenter`
/ `onmouseleave` / `onmousedown` / `onmouseup`. The same three-zone
layout applies. Use `onmousedown` on the body to emit `Pressed`.
Use `onmouseup` on a document-level listener to emit `Released`
(so the drop fires even if the cursor drifts outside the zone at
release).

```rust
// Body (Into zone + drag handle)
onmousedown: |_| on_drag.call(ItemDragMsg::Pressed(id)),
onmouseenter: |_| on_drag.call(ItemDragMsg::Entered(id, DropPosition::Into)),
onmouseleave: |_| on_drag.call(ItemDragMsg::Exited(id, DropPosition::Into)),

// Before strip
onmouseenter: |_| on_drag.call(ItemDragMsg::Entered(id, DropPosition::Before)),
onmouseleave: |_| on_drag.call(ItemDragMsg::Exited(id, DropPosition::Before)),

// After strip  
onmouseenter: |_| on_drag.call(ItemDragMsg::Entered(id, DropPosition::After)),
onmouseleave: |_| on_drag.call(ItemDragMsg::Exited(id, DropPosition::After)),

// Document-level mouseup (mounted while drag is active)
// reads current hover from tree state and emits Released or Cancelled
```

### 3.10 Tests to port

The iced test suite for ItemTree DnD lives in:

- `src/item_tree/drag/tests.rs` — 12 unit tests for validity rules,
  built against synthetic parent maps (no live tree needed). Port
  these to `dioxus-swdir-tree-core` tests; the arena makes them even
  simpler.
- `tests/item_tree_drag_drop.rs` — 15 integration tests driving the
  full state machine via the public API (`ItemDragMsg` events, then
  asserting `is_dragging()`, `drag_sources()`, `drop_target()`).
  Port to `dioxus-swdir-tree-core` tests (or the widget-level test
  if state is exposed there).

The three accessor methods (`is_dragging`, `drag_sources`,
`drop_target`) are useful for testing; add them to
`dioxus-swdir-tree-core`'s `ItemTree` if not already present.

---

## 4. Design documents updated in this release

| Document | What changed |
| --- | --- |
| `docs/src/internals/feature-specs.md` | S11.9–S11.16: ItemTree drag-and-drop spec (the oracle) |
| `docs/src/internals/state-machine.md` | New section: ItemTree state machine (all dimensions + drag transitions) |
| `docs/src/internals/porting-to-dioxus.md` | "Dependency considerations" section replaced with the share-design decision; migration checklist updated with ItemTree DnD as a `[ ]` item |
| `rfcs/done/002-item-tree-drag-and-drop.md` | Full RFC — decisions, state machine, acceptance criteria |
| `rfcs/archive/003-extract-swdir-tree-core.md` | Withdrawn RFC with rationale |

The documents `docs/src/internals/core-design.md` and
`docs/src/internals/data-model.md` still describe `DirectoryTree` only.
Both would benefit from ItemTree sections as a follow-up; that
work is deferred and does not block the DnD port.

---

## 5. Suggested RFC for `dioxus-swdir-tree`

The `dioxus-swdir-tree` project tracks its own RFCs (currently
through RFC 012 — Generic item tree). The suggested next RFC:

**RFC 013 — ItemTree drag-and-drop**

Mirror of iced RFC 002. Key points specific to the Dioxus port:

- Adopt `DropPosition { Before, Into, After }` — same semantics
  and guarantees as the iced spec.
- Validity check via arena parent links (no snapshot needed —
  simpler than iced's workaround, which is an iced-specific detail
  not a design decision).
- Add `ItemTreeEvent::Drag(ItemDragMsg)` and
  `ItemDragCompleted { sources, target, position }` (or mirror
  iced's naming exactly for spec consistency).
- Opt-in toggle on `ItemTreeView` (since `dioxus-swdir-tree-core`'s
  `ItemTree` is headless, the toggle belongs on the view component).
- The S11.9–S11.16 clauses in `iced-swdir-tree`'s
  `docs/src/internals/feature-specs.md` are the acceptance criteria.

---

## 6. What does NOT need porting

- The `iced-swdir-tree` codebase itself — no Dioxus changes needed.
- RFC 003 (withdrawn) — the decision is informational; no crate
  changes are expected.
- `swdir` version — already aligned at 0.11 on both sides.
- `ItemTree<T>` basic functionality (v0.8.0) — already implemented
  and tested in `dioxus-swdir-tree-core`.

---

## 7. Reference checklist for `dioxus-swdir-tree` maintainer

```
RFC 013 — ItemTree drag-and-drop

dioxus-swdir-tree-core:
[ ] Add DropPosition { Before, Into, After }
[ ] Add ItemDragMsg { Pressed, Entered, Exited, Released, Cancelled }
[ ] Add drag state to ItemTree (sources, hover, opt-in flag)
[ ] Implement validity check using existing arena parent links
[ ] Add drag transitions to ItemTree: on_drag_pressed, on_drag_entered,
    on_drag_exited, on_drag_released, on_drag_cancelled
[ ] Add deferred-selection: same-node release → return Completed::Click(id)
    or equivalent
[ ] Add drag accessors: is_dragging(), drag_sources(), drop_target()
[ ] Bind Escape to cancel in handle_key (only while drag active)
[ ] Port validity unit tests from iced src/item_tree/drag/tests.rs
[ ] Port state-machine integration tests from iced
    tests/item_tree_drag_drop.rs
[ ] Validate against S11.9–S11.16 in feature-specs.md

dioxus-swdir-tree (view component):
[ ] Add with_drag_and_drop(bool) or equivalent opt-in to ItemTreeView
[ ] Render three zones per row: Before strip / body / After strip
[ ] Wire onmousedown (Pressed), onmouseenter/leave (Entered/Exited)
    per zone
[ ] Mount document-level onmouseup listener during active drag;
    unmount on release or cancel
[ ] Paint insertion-bar highlight on active Before/After strip
[ ] Paint nest-here highlight on active Into body
[ ] Update examples to demonstrate reorder + nest
[ ] Update docs / README
```
