# RFC 003 — Extract a framework-agnostic `swdir-tree-core`

**Status.** Withdrawn — the extraction was decided against after discussion.
The resolved position is that the shared asset is the **design** (`docs/src/internals/`),
not a shared crate; each framework implements the spec idiomatically.
See [HANDOFF.md](../../HANDOFF.md) for the full rationale and its implications
for `dioxus-swdir-tree`.  
The Dioxus project's internal `dioxus-swdir-tree-core` split is the right
architectural call *for Dioxus*. It is not a template `iced-swdir-tree`
must copy, and `iced-swdir-tree` depending on a `dioxus-`-named crate
would be the wrong shape. A shared code crate cannot simultaneously be
idiomatic for both iced's `Task`/`ScanExecutor` model and Dioxus's
coroutine/`ScanRequest`-as-data model, and the observed representation
divergence (arena vs nested tree) confirms each architecture found its
locally-optimal structure.  
**Tracks.** Extraction of the framework-free state machine (data
model, selection, keyboard, search, drag, icons, scan protocol) out
of `iced-swdir-tree` into a standalone, UI-agnostic crate
`swdir-tree-core`, with `iced-swdir-tree` re-based as a thin iced
view layer on top. Supersedes the "Relationship to `DirectoryTree`"
deferral in RFC 001 and the `[D11]` deferral in RFC 002.  
**Touches.** New crate `swdir-tree-core`; a Cargo workspace at the
repo root; `iced-swdir-tree`'s `src/` (view layer kept, state
machine moved out); `Cargo.toml` (both crates); `docs/src/internals/`;
`CHANGELOG.md`; `ROADMAP.md`; `rfcs/README.md`. Cross-project:
informs `dioxus-swdir-tree` of a neutral core it can converge onto.

---

## Summary

`iced-swdir-tree` currently carries its own complete implementation
of the tree state machine — node model, selection, keyboard
navigation, incremental search, drag-and-drop (RFC 002), icon
themes, and the async scan protocol — entangled with the iced view
layer. This RFC extracts the framework-free part into a new crate,
`swdir-tree-core`, and re-bases `iced-swdir-tree` as a thin iced
binding (mirroring how `dioxus-swdir-tree` is a thin Dioxus binding
over `dioxus-swdir-tree-core`).

The extraction is no longer speculative: a second implementation
exists and has validated the approach.

---

## Why now (correcting RFC 001 / RFC 002 [D11])

RFC 001 deferred the core extraction "until both implementations
can be compared for convergence," and RFC 002 [D11] repeated the
deferral. Both rested on the premise that no Dioxus implementation
existed yet. **That premise is now false.**

The facts, verified against the published crates and the
`nabbisen/dioxus-swdir-tree` workspace source:

- `dioxus-swdir-tree` is published through v0.8.0. It is a Cargo
  **workspace** of two crates: `dioxus-swdir-tree` (the Dioxus view
  layer) and `dioxus-swdir-tree-core` (a framework-free state
  machine — depends only on `swdir`, no `dioxus`).
- `dioxus-swdir-tree-core`'s own docs state it was built "following
  the design documents of `iced-swdir-tree` v0.7." The extraction
  was therefore proven feasible *from this project's own specs*.
- The two implementations have **converged on spec** (identical S11
  clauses, the same selection/keyboard/search semantics, the same
  `T: Clone + Debug + Send + Sync + 'static` bounds) but **diverged
  on representation**: the Dioxus core uses a flat
  `HashMap<NodeId, InternalItem<T>>` arena with parent links and a
  `with_display(Fn(&T) -> String)` closure; `iced-swdir-tree` uses a
  nested `ItemNodeState<T>` tree with `T: Display`.

Convergence-on-spec with divergence-on-representation is exactly the
signal the deferral was waiting for: it confirms *what* is common
(the contract) and exposes *where* the seams are (the data model).
The precondition is satisfied; the deferral is retired.

A second, independent reason: `iced-swdir-tree` v0.9.0 just added
item-tree drag-and-drop, which the Dioxus core does not yet have.
Two parallel implementations are now drifting apart feature-by-feature
(this is the third feature, after the v0.7 docs and RFC 002, where
work is duplicated). A shared core stops the drift.

## Non-goals

- **Not adopting `dioxus-swdir-tree-core` directly.** Despite being
  framework-free in its dependencies, it is a member of the
  `dioxus-swdir-tree` workspace, named for and versioned in lockstep
  with the Dioxus project. An `iced-` crate depending on a
  `dioxus-`-named crate, and coupling its release cadence to the
  Dioxus workspace, is the wrong shape. The neutral name matters.
- **Not unilaterally migrating `dioxus-swdir-tree`.** That project
  converging onto `swdir-tree-core` (and retiring its own core) is
  its maintainer's call; this RFC only produces the neutral target.
- **Not a behaviour change.** The extraction preserves every
  documented behaviour and the full test suite; it is a structural
  and API-shape change, not a semantic one.

---

## Design

### [D1] A neutral crate named `swdir-tree-core`

The extracted crate is `swdir-tree-core` — neither `iced-` nor
`dioxus-` prefixed. It depends only on `swdir` (and `tempfile` as a
dev-dependency, matching the Dioxus core). It contains no UI types
(`iced::Element`, `Task`, `widget::*` stay in `iced-swdir-tree`).

It is anchored in this repository because `iced-swdir-tree` owns the
originating design documents (`docs/src/internals/`) that *both* cores
already follow. This gives `dioxus-swdir-tree` a neutral crate to
adopt later, converging the family onto one core.

### [D2] Repo becomes a Cargo workspace

```
iced-swdir-tree/                 (workspace root)
  crates/
    swdir-tree-core/             ← framework-free state machine
    iced-swdir-tree/             ← iced view layer (depends on core)
```

This mirrors the `dioxus-swdir-tree` workspace layout, satisfying
the project guideline to "introduce a Cargo workspace structure if
separating module types improves build boundaries" — which it now
demonstrably does, with a second consumer on the horizon.

### [D3] What moves into the core

Framework-free, moves to `swdir-tree-core`:
- Node model: `TreeNode`, `ItemNode`/`NodeId`, internal node state.
- `SelectionMode` + selection state machine.
- Keyboard mapping (`handle_key` as a pure function over a tree key
  enum + modifiers, returning events — *not* iced's `Key`).
- Incremental search (`SearchState`, item search).
- Drag state machines for **both** trees, including the RFC 002
  item-tree `DropPosition`/validity/transition logic.
- `IconRole`/`IconSpec`/`IconTheme`/`UnicodeTheme`/`LucideTheme`.
- The scan protocol as **data**: `ScanRequest` / `LoadPayload` /
  generation tagging, plus the `ScanExecutor` trait.

Stays in `iced-swdir-tree`:
- `view()` / `render_*` (iced `Element`, `mouse_area`, styling).
- iced `Key` → core key-enum translation in `handle_key` glue.
- The `Task`-based async wiring and `iced::widget` usage.

### [D4] API baseline: converge on the arena model

The neutral core adopts the **Dioxus core's representation** as the
baseline rather than iced's nested tree:

- `ItemTree<T>` backed by a flat `HashMap<NodeId, _>` arena with
  parent links.
- A display **closure** (`with_display(Fn(&T) -> String)`) rather
  than a `T: Display` bound.
- `ItemNode::branch(...)` / `ItemNode::leaf(...)` constructors.

Reasons:
1. The arena is the better core representation: O(1) node lookup,
   O(depth) ancestry via parent links — which makes the RFC 002
   drag validity check natural and **retires the per-drag
   parent-map snapshot** the nested model forced.
2. It is the model that is *already extracted, published, and in
   use*. Standardising on it means genuine convergence (two view
   layers, one core) instead of inventing a third shape.
3. The display closure sidesteps orphan-rule friction for foreign
   `T` and lets a host render the same `T` differently per tree.

This is the **consequential decision** of this RFC: it makes
`iced-swdir-tree`'s public `ItemTree` API change (construction and
the dropped `Display` bound). Recommended, but the explicit fork to
confirm before implementation — the alternative is to standardise on
iced's `T: Display` nested model and have the Dioxus side adapt,
which inverts who-adapts-to-whom.

### [D5] Item-tree drag-and-drop is contributed to the core

`iced-swdir-tree` v0.9.0's `DropPosition` / `ItemDragMsg` / validity
rules / state machine (RFC 002) move into `swdir-tree-core` as the
core's item-tree DnD. The validity check is rewritten against the
arena's parent links instead of a snapshot. This is the iced side's
net-new contribution to the shared core; `dioxus-swdir-tree` gains
item-tree DnD for free when it adopts the core.

### [D6] `iced-swdir-tree` re-exports for a soft landing

Where feasible, `iced-swdir-tree` re-exports core types under their
existing paths so simple downstream uses keep compiling. The
unavoidable breaks are the `ItemNode` construction change and the
`Display` → display-closure change ([D4]); these are documented with
a migration note.

### [D7] Versioning and sequencing

The extraction changes the public API shape, so it is **not** a
patch. It is its own structural release:

- `swdir-tree-core` starts at its own `0.1.0` (or is version-matched
  to the family — a packaging choice, not a design one).
- `iced-swdir-tree` takes a **minor** bump for the breaking re-base
  (e.g. `0.10.0`), landing *after* v0.9.0 and *before* the v1.0
  freeze. v1.0 then freezes a surface that is already core-backed,
  which is the right thing to freeze.

v0.9.0 (item-tree DnD) ships first and unchanged; this RFC builds on
it. The v1.0 API-freeze marker moves out by this work — correctly,
since freezing before the core extraction would freeze the wrong
architecture.

---

## Open questions

**OQ1 (the fork in [D4]).** Standardise the shared core on the
Dioxus arena model (recommended) or on iced's nested `T: Display`
model? This determines which view layer adapts and whether
`dioxus-swdir-tree` can adopt the core unchanged.

**OQ2.** Does `swdir-tree-core` version-match the family (0.x in
lockstep) or carry its own independent semver? Lockstep is simpler
for a single maintainer; independent semver is more honest about the
core's stability. Tentative: lockstep until 1.0.

**OQ3.** Migration timing for `dioxus-swdir-tree` onto
`swdir-tree-core` — in lockstep with this release, or later? Out of
scope for this repo; flagged for cross-project coordination.

---

## Acceptance criteria

1. `swdir-tree-core` builds standalone (`swdir` dep only) with the
   full state machine for both `DirectoryTree` and `ItemTree<T>`,
   including item-tree DnD ([D5]).
2. The repo is a Cargo workspace ([D2]); `iced-swdir-tree` depends on
   `swdir-tree-core` by path and contains no state-machine logic, only
   the iced view/`Task`/key-translation layer.
3. Every existing `iced-swdir-tree` test passes, relocated to whichever
   crate owns the behaviour (core-logic tests move to the core; iced
   view/wiring tests stay).
4. The full validation pipeline passes for both crates: `fmt`,
   `clippy --all-targets --all-features -D warnings`,
   `RUSTDOCFLAGS="-D warnings" cargo doc`, `cargo test --all-features`,
   `cargo publish --dry-run` (per crate).
5. A migration note documents the `ItemNode`/`Display` API changes.
6. `dioxus-swdir-tree` is left a documented, adoptable target — no
   change required in this repo.
