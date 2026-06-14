# Changelog

All notable changes to `iced-swdir-tree` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the crate follows [Semantic Versioning](https://semver.org/).

## [0.9.1] — 2026-06-14

Audit-driven patch release. No new public API; no behaviour changes
for code not exercising the fixed edge case.

### Fixed

- **`set_tree` now clears in-flight drag state.** If `set_tree` was
  called while a drag was active the parent-map snapshot (built at
  press time) became stale, causing subsequent hover-validity checks
  to run against the old tree structure. The widget now clears
  `self.drag` at the start of every `set_tree` call.
  (`docs/design/state-machine.md` already specified this behaviour
  in the composability table; the code was wrong.)

### Removed

- `TreeNode::node_count()` — internal helper, never called outside
  its own definition. Deleted to reduce the dead surface.
- `TreeCache::clear()` — same.

### Internal

- Removed a spurious `#[allow(dead_code)]` annotation on
  `directory_tree::node::VisibleRow::depth`. The field is actively
  read by `keyboard.rs` (lines 195, 202); the attribute was hiding
  a live field behind a silenced warning.
- Corrected the `ItemTree` module-doc comparison table: the
  drag-and-drop row still read "Deferred to v0.8.x" after RFC 002
  shipped in v0.9.0.
- Added three integration tests covering previously-untested
  composability rules and spec clause S11.16:
  `set_tree_while_drag_active_clears_drag`,
  `disabling_dnd_while_drag_active_clears_drag`,
  `set_search_query_preserves_drag_state`.

---

## [0.9.0] — 2026-06-09

Implements **RFC 002 — Drag-and-drop for `ItemTree<T>`**. Resolves
the deferral in RFC 001 [D10] ("descendant-validity check requires
explicit tree traversal") and the open `Escape`-during-drag note in
RFC 001 [D7] for `ItemTree`. Drag-and-drop is opt-in and off by
default; existing `ItemTree` applications are unaffected.

### Added

- **`DropPosition`** — `Before` / `Into` / `After`, describing where
  a dragged node lands relative to the drop target. Each maps to an
  unambiguous `(new_parent, insertion_point)` without depending on
  cursor geometry.
- **`ItemDragMsg`** — opaque drag-machinery event enum for `ItemTree`,
  mirroring `DragMsg` for `DirectoryTree`. Applications route these
  back through `update` unchanged.
- **`ItemTreeEvent::Drag(ItemDragMsg)`** — internal drag instrumentation
  event, analogous to `DirectoryTreeEvent::Drag`.
- **`ItemTreeEvent::DragCompleted { sources, target, position }`** —
  emitted when the user drops on a valid target. `sources` is a
  non-empty `Vec<NodeId>` in tree order; `target` is live; the
  `(target, position)` pair satisfied the validity rules.
- **`ItemTree::with_drag_and_drop(bool)`** — opt-in builder (default
  off). When off, view behaviour is byte-identical to v0.8.
- **`ItemTree::is_dragging()`**, **`drag_sources()`**,
  **`drop_target()`** — read-only drag-state accessors.
- **Drop-zone view**: when enabled, each row gets a thin `Before`
  strip, the row body (the `Into` zone), and a thin `After` strip.
  A primary-palette insertion bar lights up on the active strip;
  the `Into` zone shows the same success-palette drop highlight as
  `DirectoryTree`.
- **`Escape` cancels** an in-flight `ItemTree` drag (only while a
  drag is active, so applications keep `Escape` otherwise). Closes
  the RFC 001 [D7] open item.
- **Validity via parent-map snapshot**: when a drag begins, the widget
  takes an O(n) `HashMap<NodeId, Option<NodeId>>` snapshot. Hover
  validity checks are then O(depth) chain walks — the `NodeId`
  analogue of `DirectoryTree`'s free O(1) `PathBuf::starts_with`.
- **15 new integration tests** in `tests/item_tree_drag_drop.rs`
  covering: idle state, disabled-DnD noop, press, hover/exit, invalid
  target, Escape-cancel, Escape-idle-unbound, release-clears-state,
  same-node click, multi-source tree order, unselected-node drag,
  cancelled event, and stray-entered noop.
- **12 new unit tests** in `src/item_tree/drag/tests.rs` covering the
  validity rules: self-drop, descendant cycle, deep cycle, sibling
  reorder, nest into unrelated node, root-sibling rejection,
  nest-into-root, drop-into-current-parent, nonexistent target,
  multi-source valid, multi-source cycle, target-in-sources.
- **RFC 002** filed at `rfcs/done/002-item-tree-drag-and-drop.md`.
- **`examples/item_tree.rs`** updated: enables `with_drag_and_drop`,
  handles `DragCompleted` with a full `apply_move` that extracts
  sources, inserts at the requested position, and calls
  `set_tree_and_recompute_search`. Propagates the widget's `Task`
  correctly (v0.8 dropped it — fine for v0.8's event set, but
  necessary for the deferred-selection click and drop events).

### Changed

- **`swdir` dependency bumped `0.10` → `0.11`** (resolves to 0.11.3),
  aligning with `dioxus-swdir-tree-core`. No source changes were
  required; the only public touchpoint is `From<&swdir::ScanError>
  for Error`, so downstreams that construct the crate's `Error` from
  a `swdir::ScanError` will need `swdir 0.11` as well.
- **`docs/design/feature-specs.md`** S11.9 updated from "out of scope
  for v0.8" to the shipped drag spec (S11.9 – S11.16).
- **`docs/guide/drag-and-drop.md`** gains a complete `ItemTree`
  drag-and-drop section.
- **`ROADMAP.md`** slots v0.9.0 before the v1.0 API-freeze (v1.0 will
  now freeze the surface that includes `ItemTree` drag-and-drop).

### No breaking changes to `DirectoryTree`

All existing `DirectoryTree` behaviour is unchanged.

---

## [0.8.0] — 2026-06-09

Implements **RFC 001 — Generic item tree**. Adds `ItemTree<T>`,
a sibling to `DirectoryTree` that provides the same keyboard
navigation, multi-select, expand/collapse, and search surface
for caller-supplied, in-memory node data. No async I/O, no
generation counter, no filesystem dependency.

### Added

- **`NodeId(u64)`** — an opaque caller-assigned node identity.
- **`ItemNode<T>`** — the caller-facing input struct: `{ id,
  data: T, children: Vec<ItemNode<T>> }`.
- **`ItemTree<T>`** — the widget, parametrised over node data
  type `T: Clone + Debug + Send + Sync + 'static`.
  - `ItemTree::new()` — constructs an empty tree.
  - `set_tree(root: ItemNode<T>)` — populate or replace the
    tree with key-based diffing.
  - `set_tree_and_recompute_search(root)` *(requires `T:
    Display`)* — same as `set_tree` plus re-runs any active
    search query.
  - All selection methods: `selected_ids()`,
    `is_selected(NodeId)`, `active_id()`, `anchor_id()`.
  - `update(ItemTreeEvent) -> Task<ItemTreeEvent>` — always
    returns `Task::none()`.
  - `handle_key(&Key, Modifiers) -> Option<ItemTreeEvent>` —
    identical bindings to `DirectoryTree`.
  - `set_search_query(q)` *(requires `T: Display`)*,
    `clear_search()`, `is_searching()`,
    `search_query()`, `search_match_count()` — same
    semantics as `DirectoryTree`.
  - `view(mapper) -> Element` *(requires `T: Display`)*.
  - `with_icon_theme(Arc<dyn IconTheme>)` — same interface.
- **`ItemTreeEvent`** — `Toggled(NodeId)` and `Selected(NodeId,
  SelectionMode)`. No `Loaded` variant.
- **`examples/item_tree.rs`** — a section-outline demo using
  `ItemTree<Section>` with re-parse, search, and multi-select.
- **21 new unit tests** in `src/item_tree/tests.rs` covering
  construction, expand/collapse, selection (all three modes),
  key-based diffing (expansion preserved / selection
  preserved / disappeared ids dropped / position-change
  preserves state), search (filter + ancestors, case-
  insensitive, empty clears, selection survives), keyboard
  navigation, icon theme, and object-safety.
- **RFC 001** filed at `rfcs/proposed/001-generic-item-tree.md`
  with the `rfcs/README.md` index.

### Key-based diffing contract

`set_tree` diffs the new `ItemNode<T>` tree against the
current internal tree using `NodeId` as the key:

- **Surviving keys**: expansion and selection state are
  preserved regardless of position change.
- **New keys**: start collapsed and unselected.
- **Disappeared keys**: silently removed from `selected_ids`,
  `active_id`, and `anchor_id`.

This is the design the `layered` (Dioxus Markdown editor)
author requested: re-parsing the document on every keystroke
rebuilds the tree, but the user's open sections and selections
persist across edits.

### What `ItemTree<T>` does NOT have

Compared to `DirectoryTree`, the following are absent by design:

- No async I/O (`Task::none()` always).
- No `ScanExecutor`, no generation counter.
- No `DirectoryFilter` (per-node filtering can be expressed by
  omitting nodes from the input tree).
- No drag-and-drop (deferred to v0.8.x — descendant validity
  requires explicit tree traversal rather than path-prefix
  comparison).

### Test counts

- **174 total** (was 154): 109 unit + 65 integration + 0
  doctest. Added 21 unit tests for `ItemTree`.

## [0.7.2] — 2026-06-07

**Design documents for `dioxus-swdir-tree` and future ports.
No code changes, no test changes.**

### Added — `docs/design/`

A new `docs/design/` subdirectory ships five framework-agnostic
specification documents written to support development of
`dioxus-swdir-tree` (and any other widget port targeting a
different UI framework). Together they cover:

- **[`core-design.md`](docs/design/core-design.md)** — the ten
  defining properties of the widget: what it is, what it is
  not, the non-blocking I/O model, the generation-tag
  protocol, why selection is by-path rather than by-node, the
  hard line between widget-owned UI state and app-owned data
  state, orthogonality of the four state dimensions, and the
  scan lifecycle end-to-end.

- **[`data-model.md`](docs/design/data-model.md)** — every
  field of `DirectoryTree` with its type shape, invariants,
  and derivation rules. Covers `TreeNode` (five named
  invariants), `TreeCache` and why it stores unfiltered
  entries, the generation counter (what bumps it and what
  does not), the three selection fields and their exact
  semantics, `DragState`, `prefetching_paths`,
  `SearchState`, and `icon_theme`.

- **[`state-machine.md`](docs/design/state-machine.md)** —
  precise transition specification for every event, written
  as pseudocode condition-action pairs: `Toggled` (four
  cases), `Loaded` (seven steps), `Selected` (all three
  `SelectionMode` values), all five `Drag::*` variants,
  `set_filter`, `set_search_query`, `clear_search`. Includes
  the `visible_rows()` algorithm and a composability table
  for all cross-dimension state combinations.

- **[`feature-specs.md`](docs/design/feature-specs.md)** —
  numbered `S<n>.<m>` behavioural clauses for all ten
  features, written as a test oracle. Covers lazy loading,
  display filters, single-select, keyboard navigation,
  pluggable executor, multi-select, drag-and-drop, parallel
  pre-expansion, incremental search, and icon themes.
  Notable: the error-node retry rule (S1.6), the
  `FoldersOnly` hides hidden-directories rule (S2.2), and
  the press-release-same-row = click rule (S7.2).

- **[`porting-to-dioxus.md`](docs/design/porting-to-dioxus.md)** —
  concrete iced → Dioxus translation with code sketches:
  async scanning via `use_coroutine`, prefetch fan-out,
  keyboard via `onkeydown`, drag-and-drop synthesised from
  mouse events (and why HTML5 drag-and-drop is unsuitable),
  and a `swdir-tree-core` extraction proposal that would let
  both `iced-swdir-tree` and `dioxus-swdir-tree` share the
  state machine and tests without iced being a transitive
  dependency of Dioxus apps. Ends with an 11-step migration
  checklist.

### Not changed

- **Public API byte-identical to 0.7.0.** No new types,
  no renamed methods, no behaviour changes.
- **Tests unchanged.** Still 154 tests, still all green.
- The `docs/design/README.md` index was also added as the
  entry point to the new subdirectory.

## [0.7.1] — 2026-04-30

Dependencies updated.

## [0.7.0] — 2026-04-24

**Final v1.0 gate: custom icon themes via a trait.** With this
release the widget's last hard-coded dependency (lucide icons)
becomes pluggable. The API surface is now frozen — v1.0 will
follow as a version-number-only release.

### Added

- **`IconTheme` trait** (object-safe, `Send + Sync + Debug`)
  with a single method:
  ```rust
  fn glyph(&self, role: IconRole) -> IconSpec;
  ```
- **`IconRole`** — `#[non_exhaustive]` enum of six semantic
  positions: `FolderClosed`, `FolderOpen`, `File`, `Error`,
  `CaretRight`, `CaretDown`. Future minor releases may add
  variants (`Symlink`, `Hidden`, `Loading`, …), so external
  themes must include a `_ =>` fallback when matching.
- **`IconSpec`** — the data an `IconTheme` returns per role:
  `glyph: Cow<'static, str>`, `font: Option<iced::Font>`,
  `size: Option<f32>`. Public fields for `const`-style themes;
  `new` / `with_font` / `with_size` builder methods for
  ergonomics.
- **`UnicodeTheme`** — always-available stock theme rendering
  short Unicode symbols (📁 📂 📄 ⚠ ▸ ▾) in the default system
  font.
- **`LucideTheme`** — stock theme rendering real lucide vector
  glyphs via the bundled `LUCIDE_FONT_BYTES` TTF. Gated on the
  `icons` feature.
- **`DirectoryTree::with_icon_theme(Arc<dyn IconTheme>)`** —
  builder to plug in a custom theme.
- **`examples/icon_theme.rs`** — a three-way theme switcher
  (Unicode / Label / Ascii) demonstrating the full trait.
- **8 new unit tests** in `src/directory_tree/icon/tests.rs`
  (stock-theme glyphs, no-font-on-UnicodeTheme, lucide font
  set, single-char lucide glyphs, IconSpec builder,
  `Into<Cow<'static, str>>` accepts both variants, custom theme
  implementation, object-safety, default-theme completeness).
- **2 additional unit tests** gated on `icons` covering
  `LucideTheme` behaviour.
- **5 new integration tests** in `tests/icon_theme.rs`
  (default-theme-installed, `Arc<dyn>` accepted, view calls the
  installed theme via a `CountingTheme` fake, theme survives
  filter change, `Arc` is cheaply cloneable).

### Changed

- **`DirectoryTree` gained a `icon_theme: Arc<dyn IconTheme>`
  field.** Initialized via `icon::default_theme()` in `new()`
  to the stock theme for the feature configuration
  (`LucideTheme` with `icons`, `UnicodeTheme` without).
- **`src/directory_tree/view.rs`** threads `&dyn IconTheme`
  through `render_node` and `render_row`. The old feature-gated
  `render_text` / `render_lucide` fallbacks were deleted —
  `icon::render(theme, role)` is the single dispatch point.
- **The `icons` feature's purpose shrinks.** It now controls
  whether `LucideTheme` + the lucide TTF are pulled in; apps
  that plug in their own theme can turn it off for a slimmer
  binary:
  ```toml
  iced-swdir-tree = { version = "0.7", default-features = false }
  ```

### Public API decisions (frozen for v1.0)

- **Trait returns data, not widgets.** `IconSpec` is plain
  fields, not an `Element`; keeps the trait object-safe and
  lets the widget own layout/sizing.
- **`Arc<dyn IconTheme>`, not `Box<dyn>`.** Matches the
  `Arc<dyn ScanExecutor>` convention already in the crate.
- **`IconRole` is `#[non_exhaustive]`.** Future-proofs the role
  set for `Symlink` / `Hidden` / `Loading` additions without
  breaking existing themes.
- **`IconSpec` is NOT `#[non_exhaustive]`.** The three-field
  shape is frozen for v1.0; adding fields later would be a
  breaking 2.0 change.

### Breaking changes

None to downstream apps that use the builder/API. The internal
`Icon` enum was made public as `IconRole` and the
feature-gated `render` helpers were consolidated behind the
trait — but these were never part of the public API.

### Test counts

- **154 total** (was 141): 88 unit + 65 integration + 1 doctest.
  Added 8 unit tests, 2 feature-gated unit tests, and 5
  integration tests.

## [0.6.3] — 2026-04-24

**Documentation reorganization. No code changes, no test changes.**

### Rationale

The v0.6.2 release split `README.md` into a `docs/` folder with
12 flat files and mixed filename casing (`ARCHITECTURE.md` /
`DEVELOPMENT.md` vs `configuration.md`). Flat is fine for a
half-dozen files, but twelve at one depth makes "what do I read
first?" hard to answer. This release groups pages by reader
intent and unifies naming.

### Changed — `docs/` now has three subfolders

Task-oriented split by what the reader is trying to do:

- **`docs/guide/`** — *"I want to build something."* Pages you
  copy code from.
  - `configuration.md`, `multi-select.md`, `drag-and-drop.md`,
    `keyboard-navigation.md`, `incremental-search.md`,
    `prefetch.md`, `custom-executor.md`.
- **`docs/reference/`** — *"I want to look up a fact."* Short
  pages; once you've scanned them, you're done.
  - `features.md`, `events.md`.
- **`docs/internals/`** — *"I want to understand how this works,
  or contribute."* For maintainers and curious readers.
  - `architecture.md`, `development.md`.

`docs/README.md` is unchanged in role — GitHub auto-renders it
when you navigate into the folder — but it now acts as an index
grouped by the three buckets above, each with one-line
descriptions.

### Renamed files

Lowercase-kebab-case throughout, and a couple of clearer names:

| Before                | After                                    |
| ---                   | ---                                      |
| `docs/ARCHITECTURE.md` | `docs/internals/architecture.md`        |
| `docs/DEVELOPMENT.md`  | `docs/internals/development.md`         |
| `docs/configuration.md`| `docs/guide/configuration.md`           |
| `docs/multi-select.md` | `docs/guide/multi-select.md`            |
| `docs/drag-and-drop.md`| `docs/guide/drag-and-drop.md`           |
| `docs/keyboard.md`     | `docs/guide/keyboard-navigation.md`     |
| `docs/search.md`       | `docs/guide/incremental-search.md`      |
| `docs/prefetch.md`     | `docs/guide/prefetch.md`                |
| `docs/executor.md`     | `docs/guide/custom-executor.md`         |
| `docs/features.md`     | `docs/reference/features.md`            |
| `docs/events.md`       | `docs/reference/events.md`              |

### Link maintenance

Every relative link inside `docs/` and the top-level `README.md`
was rewritten to match the new hierarchy. A link-checker script
confirmed no broken references remain across the entire
documentation set before release.

### Design decisions

- **Three buckets, not four.** The Diátaxis framework (tutorial
  / how-to / reference / explanation) would over-fit a 12-file
  crate; the three-way split maps to actual reader audiences.
- **Descriptive filenames.** `executor.md` became
  `custom-executor.md` so the topic is clear at a glance;
  `keyboard.md` became `keyboard-navigation.md` to match the
  page's own heading.
- **`features.md` is reference, not guide.** Its bullets are a
  scan-once feature list. Each bullet links to the `guide/`
  page for implementation details.
- **`events.md` is reference.** Short enum description, not a
  how-to.

### Not changed

- **Public API is byte-identical to 0.6.2.** No new types, no
  renamed methods, no behaviour changes.
- **Tests unchanged.** Still 141 tests, still all green.

## [0.6.2] — 2026-04-24

**Documentation restructure. No code changes, no test changes.**

### Rationale

`README.md` had grown to just over 500 lines — readable only if
you already knew what you were looking for, and heavy for the
crates.io landing page. The fix is standard practice: keep the
README as a concise hero + overview + quick start + pointers,
and move each topic into its own page under `docs/`.

### Changed

- **`README.md` → 140 lines.** Now a pure landing page:
  GitHub-style hero with badges, overview, "when to use it",
  quick start (install + minimal app + icons feature), design
  notes, and a curated link index into `docs/` plus
  `CHANGELOG.md` / `ROADMAP.md`.
- **`docs/` directory added with 10 topic pages:**
  - `docs/README.md` — index (lands here when GitHub renders
    the folder).
  - `docs/features.md`, `docs/configuration.md`,
    `docs/events.md`.
  - `docs/multi-select.md`, `docs/drag-and-drop.md`,
    `docs/keyboard.md`.
  - `docs/prefetch.md`, `docs/search.md`, `docs/executor.md`.
  - `docs/ARCHITECTURE.md` and `docs/DEVELOPMENT.md` — moved
    from repo root. Cross-links to `CHANGELOG.md` / `ROADMAP.md`
    updated to use `../` relative paths.

Each topic page is self-contained (runnable examples, full
semantics, all relevant links) so direct landings from docs.rs
search results or external bookmarks stay useful. Content is
preserved verbatim — no edits except heading-level adjustments
and inter-page link rewrites.

### Cleaned up

- **README license section now matches `Cargo.toml`.** The
  previous README claimed a dual MIT-OR-Apache-2.0 license and
  linked `LICENSE-MIT` / `LICENSE-APACHE` files that don't
  exist; `Cargo.toml` specifies Apache-2.0 only. Fixed the
  badge and the section to reflect reality.

### Not changed

- **Public API is byte-identical to 0.6.1.** No new types, no
  renamed methods, no behaviour changes. Downstream apps that
  compile against 0.6.1 compile against 0.6.2 with no edits.
- **Tests unchanged.** Still 141 tests (80 unit + 60
  integration + 1 doctest), still all green.

## [0.6.1] — 2026-04-24

Adds a **safety valve for v0.5 prefetch**: the widget now refuses
to speculatively scan directories whose basenames appear in a
configurable skip list. The default list covers the usual
suspects — version-control metadata (`.git`, `.hg`, `.svn`),
JavaScript (`node_modules`), Python (`__pycache__`, `.venv`,
`venv`), Rust/Java (`target`), and generic build output (`build`,
`dist`).

### Why

v0.5 prefetch eagerly scans folder-children of any user-expanded
folder. On a typical Rust/Node project root that means
speculatively scanning `.git/objects/` (tens of thousands of
tiny files) and `node_modules/` (potentially hundreds of
thousands) on the first expansion — a large I/O cost for content
the user was almost certainly not browsing toward. 0.6.1 closes
that trap.

### Added

- **`DEFAULT_PREFETCH_SKIP`** — public `&[&str]` constant with
  the default skip list. Re-exported from the crate root so apps
  can read it and extend.
- **`DirectoryTree::with_prefetch_skip<I, S>(I) -> Self`** where
  `I: IntoIterator<Item = S>, S: Into<String>` — replace the skip
  list (default is populated from `DEFAULT_PREFETCH_SKIP`). Pass
  an empty iterator to disable skipping entirely.
- **`TreeConfig::prefetch_skip: Vec<String>`** — the field that
  actually holds the list. `pub` so tests/tooling can introspect.
- **7 new unit tests** (`default-skips-dot-git-and-target`,
  `custom-list-replaces-defaults`, `empty-list-disables`,
  `case-insensitive-ascii-match`, `exact-basename-not-substring`,
  `user-click-still-scans-skipped`,
  `default-const-matches-default-field`).
- **6 new integration tests** (`default-prevents-dot-git`,
  `default-prevents-node_modules-and-target`,
  `user-click-scans-skipped`, `custom-replaces-defaults`,
  `empty-disables`, `const-is-reexported`).

### Behaviour change (patch-level)

On `0.6.0`, an app with `with_prefetch_limit(N)` enabled over a
repo root would see its `.git/`, `node_modules/`, and `target/`
directories silently prefetched. On `0.6.1`, they are skipped by
default. Apps that actually *want* `.git/` prefetched — e.g. a
dedicated git-objects browser — must opt in explicitly with
`.with_prefetch_skip(Vec::<String>::new())` or a custom list
that excludes it.

This is a deliberate strictly-safer default. No public API is
removed; no downstream code that compiled against 0.6.0 fails to
compile against 0.6.1.

### Matching rules (documented)

- **Exact basename match**, not substring. `"target"` skips
  `target/` and `Target/` but not `my-target-files/`.
- **ASCII case-insensitive.** Picks up `.Git/` on case-
  insensitive filesystems (macOS HFS+, Windows NTFS) without the
  app having to list every casing.
- **Prefetch only.** A user click on a skipped folder still
  expands it normally. The skip list governs automatic behaviour,
  not user actions.

### Test counts

- **141 total** (was 128): 80 unit + 60 integration + 1 doctest.
  Added 7 unit and 6 integration tests for the safety valve.

## [0.6.0] — 2026-04-24

Delivers the v1.0-required roadmap item: **incremental search
with real-time filtering**. Apps that host a `DirectoryTree` can
now wire a `text_input` directly into the widget via
`tree.set_search_query(...)`: as the user types, rows whose
basenames don't match are hidden, but their ancestors stay
visible so the match's tree context is preserved.

### Added

- **`DirectoryTree::set_search_query(impl Into<String>)`** — the
  primary entry point. Case-insensitive substring match on each
  node's basename. Passing `""` is equivalent to `clear_search()`.
- **`DirectoryTree::clear_search()`** — drop the active query.
- **`DirectoryTree::search_query() -> Option<&str>`** — the
  current query as the app set it (original casing preserved for
  status-bar display).
- **`DirectoryTree::is_searching() -> bool`** — convenience
  accessor.
- **`DirectoryTree::search_match_count() -> usize`** — count of
  direct matches (ancestor-breadcrumb rows are not counted), for
  apps that want to show "N matches".
- **New `examples/search.rs`** demonstrating text-input + tree +
  status bar + expand-all button pattern.
- **6 new unit tests** in `src/directory_tree/search/tests.rs`
  covering the `matches_query` primitive (empty / basename
  substring / case-insensitivity / path-components-don't-match /
  no-match / query-longer-than-basename).
- **9 new integration tests** in `tests/search.rs` against real
  filesystem fixtures — inactive-by-default, multi-subtree
  matching, empty-clears, clear-restores, case-insensitive,
  selection-preservation, filter-change-re-runs, sees-through-
  collapsed-loaded-subtree, on-loaded-recomputes.

### Changed — internals

- **`TreeNode::visible_rows` gets a wrapper on `DirectoryTree`.**
  `DirectoryTree::visible_rows()` now dispatches: no search → old
  `is_expanded`-respecting walker; search active → new walker
  that consults the cached `SearchState::visible_paths` set
  instead (so ancestors-of-matches render regardless of
  `is_expanded`). Both keyboard nav and view rendering now go
  through this wrapper, so search-mode is consistent everywhere.
- **`view::render_node` signature** gained a
  `search_visible: Option<&HashSet<PathBuf>>` parameter. When
  present, the recursion skips non-visible nodes and ignores
  `is_expanded`.
- **`on_loaded` and `set_filter`** now trigger
  `recompute_search_visibility()` so newly-loaded children
  auto-appear in active searches and filter flips re-run the
  match set without the app re-issuing `set_search_query`.

### Semantics decisions (documented)

- **Already-loaded nodes only.** Search never triggers new
  filesystem scans. Apps that want broad coverage should combine
  search with the v0.5 `with_prefetch_limit(N)` option.
- **Sees through collapse.** A loaded-but-collapsed folder still
  contributes its matches; the ancestor chain is force-rendered.
- **Selection is orthogonal.** Selected rows hidden by a search
  stay selected and reappear when the search clears.
- **No auto-expand on click during search.** Clicking to expand a
  folder while a search is active does not escape the filter —
  the view stays narrowed to matches-and-ancestors. Clearing the
  query first is the documented way to "explore outside current
  results". A future 0.6.x patch can add an opt-in escape if
  demand materializes.
- **Empty string clears search** (two-state machine, not three).

### Breaking changes

None on the public API. `TreeConfig` is unchanged. New methods
only.

### Test counts

- **128 total** (was 113): 73 unit + 54 integration + 1 doctest.
  Added 6 unit and 9 integration tests for search.

## [0.5.0] — 2026-04-24

Delivers the third v1.0-required roadmap item: **parallel
pre-expansion of visible descendants**. When a user expands a
folder, the widget can now speculatively issue background scans
for the folder's direct children-that-are-folders, in parallel via
the existing [`ScanExecutor`] trait. When the user next clicks to
expand one of those children, no I/O happens — the data is
already cached. Apps with a fast executor (tokio / smol / rayon)
get real concurrency for free.

### Added

- **`TreeConfig::prefetch_per_parent: usize`** — caps how many
  folder-children to prefetch when a parent's scan completes. `0`
  (the default) disables prefetch and matches v0.1–0.4 behaviour
  exactly. See the field docs for the full contract.
- **`DirectoryTree::with_prefetch_limit(usize) -> Self`** —
  builder method matching `with_filter` / `with_max_depth` /
  `with_executor` style.
- **`DirectoryTree::prefetching_paths`** — `pub(crate)` state
  tracking paths whose prefetch-triggered scans are in flight.
  Used by `on_loaded` to prevent cascade and by `on_toggled` to
  upgrade a pending prefetch to a user action when the user
  clicks to expand a path that's about to prefetch-load.
- **Six new integration tests** in `tests/prefetch.rs` covering
  the baseline (disabled), folder-children-only, one-level-deep
  no-cascade, `max_depth` interaction, the limit cap, and the
  prefetched-click-is-instant fast path.
- **Seven new unit tests** in `update/tests.rs` covering
  `select_prefetch_targets` edge cases and the cascade-prevention
  machinery.

### Changed — contract and cascade prevention

- **`on_loaded` signature**: now returns `Vec<PathBuf>` — the
  paths the widget wants scanned next as prefetch targets. The
  `update()` dispatcher converts that Vec into a `Task::batch` of
  scan tasks. This keeps handlers as pure state transitions and
  task emission centralized in the dispatcher.
- **`on_toggled`**: when the user expands a path that's currently
  being prefetched, the prefetch flag is cleared so the eventual
  fresh scan is treated as a user-initiated load (and triggers
  its own prefetch wave). The stale prefetch result will arrive
  with a mismatched generation and be dropped by the existing
  generation check.
- **`__test_expand_blocking`**: now also drains the prefetch wave
  synchronously when prefetch is enabled. Integration tests that
  used to assert on "after expanding /r, only /r's children are
  loaded" still pass — they use the default `prefetch_per_parent
  = 0`. Tests that explicitly opt in via `.with_prefetch_limit(N)`
  see folder-grandchildren populated after a single call.

### Breaking change (pre-1.0 minor bump)

- **`TreeConfig` gained a public field** (`prefetch_per_parent`).
  External code that constructs `TreeConfig` by value — rather
  than using the `DirectoryTree::new`/`with_*` builder chain —
  needs to add the new field (or use `..Default::default()` if it
  derives Default on its own wrapper). Per semver's pre-1.0
  allowance, minor-version bumps may break; the overwhelming
  majority of apps construct `DirectoryTree` via the builder and
  are unaffected.

### Limitations documented

- **One-level-deep only.** A folder loaded via prefetch does not
  itself trigger further prefetch scans. Cascading prefetch is
  exponential (`per_parent ^ depth`) and is not appropriate as a
  default. Apps that want deeper behaviour can either issue
  further `Toggled` events from their update handler, or bump
  `per_parent` very high and accept that user clicks still do one
  round-trip per level.
- **No global concurrency cap.** Each user-initiated expansion
  bursts up to `prefetch_per_parent` scans into the executor.
  With the default `ThreadExecutor` (one `std::thread::spawn` per
  scan), a `prefetch_per_parent = 50` setting on a 20-child
  folder means 50 threads in flight at once. Apps on the default
  executor should keep `per_parent` modest (5–25). Apps that have
  plugged in a bounded tokio/smol pool don't have this problem —
  the pool queues excess tasks.
- **Prefetch doesn't auto-expand.** `is_loaded = true` but
  `is_expanded = false`. The user still controls what's drawn
  on screen; prefetch only makes the eventual expand instant.

### Test counts

- **113 total** (was 100): 67 unit + 45 integration + 1 doctest.
  Added 7 unit and 6 integration tests for prefetch.

## [0.4.2] — 2026-04-24

**Pure refactor release. No behaviour changes, no public API
changes. The same 100 tests run, with the same names, across the
new layout.**

Where [0.4.1](#041--2026-04-24) split up the `src/` tree, this
release applies the same principle to `tests/`. The two existing
integration files had each grown past 400 lines with several
well-defined themes each: `integration.rs` (430 lines) mixed
filter modes, expand/collapse, selection, runtime filter flips,
and error paths into one binary; `tree.rs` (819 lines) layered
v0.2, v0.3, and v0.4 sections plus an unlabeled catch-all
"Tests" block on top.

### Changed — test layout only

- **Shared fixtures moved to `tests/common/mod.rs`.** `TmpDir` and
  the tree-introspection helpers (`child_names`, `find_in_tree`,
  `is_root`) used to be re-declared at the top of each integration
  file — now they live in one place. Rust's test harness treats
  subdirectories under `tests/` as shared code, not as independent
  test binaries, so `mod common;` at the top of each test file
  pulls them in without spinning up another compilation target.
- **`tests/integration.rs` → 5 themed files:**
  - `filter_modes.rs` — the three `DirectoryFilter` variants (3 tests)
  - `expand_collapse.rs` — expand, collapse, re-expand round-trip (2 tests)
  - `selection_basic.rs` — single-select under `SelectionMode::Replace` (1 test)
  - `runtime_filter.rs` — `set_filter` changing visibility without a
    rescan (1 test)
  - `error_paths.rs` — nonexistent path, unknown Toggled target,
    permission-denied (3 tests)
- **`tests/tree.rs` → 7 themed files:**
  - `tree_filters.rs` — the four filter-mode tests against real FS
    (4 tests)
  - `tree_filter_preservation.rs` — state-preservation across filter
    flips and collapse/re-expand cycles (3 tests)
  - `tree_selection.rs` — single-selection behaviour with filter
    interaction (3 tests)
  - `tree_errors.rs` — error surfacing on nonexistent path and
    permission denial (2 tests)
  - `tree_executor.rs` — v0.2 custom `ScanExecutor` plumbing
    (2 tests)
  - `tree_multi_select.rs` — v0.3 `SelectionMode` matrix against
    real FS trees (4 tests)
  - `tree_drag_drop.rs` — v0.4 drag-and-drop state-machine
    invariants (11 tests)

### File size impact

| | 0.4.1 | 0.4.2 (max) |
|---|---|---|
| Largest test file | `tree.rs` (819) | `tree_drag_drop.rs` (~210) |
| Test binaries | 2 | 12 + common helper |

Every test file now sits under ~210 lines; most are under 120.

### Risk summary

Same as 0.4.1 — contents were moved, not rewritten. Full test
matrix (`cargo test`, `cargo test --all-features`,
`cargo clippy --all-targets --all-features -- -D warnings`,
`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`,
`cargo publish --dry-run`) is clean. Nothing on the public API
surface changed; downstream apps need not bump their code.

## [0.4.1] — 2026-04-24

**Pure refactor release. No behaviour changes, no public API changes,
no new or removed tests — the 100-test suite is identical in structure
and passes verbatim against 0.4.0 semantics.**

The motivation was file size: `update.rs` had grown to 937 lines, and
`keyboard.rs` to 542, each mixing production code with a large
end-of-file `#[cfg(test)] mod tests { ... }` block. Both thresholds
make it harder to navigate one handler without scrolling past the
others, and they hide the tests from anyone grepping for a specific
test name.

### Changed — internal layout only

- **Every inline `mod tests` block is now a sibling file.** Seven
  modules now declare `#[cfg(test)] mod tests;` and carry their
  tests in a `<module>/tests.rs` file: `config`, `drag`,
  `keyboard`, `node`, `selection`, `update`, and `walker`. Rust
  2018+ supports `foo.rs` living next to a `foo/` directory of
  submodules, so this keeps the crate-root namespace exactly as it
  was. No test names changed; the same 60 unit tests run in the
  same places.
- **`update.rs` split into a dispatcher + four handler
  submodules.** The `update` module now owns only the `update()`
  dispatch match, the shared `depth_of` helper, and
  `__test_expand_blocking`. Each event's logic lives in its own
  file:
  - `update/on_toggled.rs` — expand/collapse, triggers scans.
  - `update/on_selected.rs` — the three `SelectionMode` branches
    plus the `compute_visible_range` helper.
  - `update/on_drag.rs` — the v0.4 drag state machine.
  - `update/on_loaded.rs` — async-scan result merge, plus the
    private `build_children` helper.

  Handler methods are now `pub(super) fn on_xxx` so the parent
  dispatcher can still call them; they were previously private.
  This is a visibility loosening within the crate only — the
  public API is identical.
- **`keyboard.rs` tests extracted** without splitting the
  production module itself. The `handle_key` dispatcher and its
  action helpers are a single cohesive feature (per-key match arm
  helpers), and splitting them would have been arbitrary. Tests
  in `keyboard/tests.rs` (281 lines) now sit cleanly alongside.

### File size impact

| File | 0.4.0 | 0.4.1 |
|---|---|---|
| `update.rs` | 937 | 136 |
| `keyboard.rs` | 542 | 262 |
| Largest test file | `update.rs` (478 inline) | `update/tests.rs` (481) |
| Largest production file | `update.rs` (459) | `view.rs` (285) |

No file above 300 lines of production code remains.

### Risk summary

This release touches 60 % of the crate by file, but every change is
mechanical file-placement: contents were moved, not rewritten. The
full test matrix (`cargo test`, `cargo test --all-features`,
`cargo clippy --all-targets --all-features -- -D warnings`,
`RUSTDOCFLAGS="-D warnings" cargo doc`,
`cargo publish --dry-run --all-features`) is clean.

## [0.4.0] — 2026-04-24

Delivers the second of the five v1.0-required roadmap items:
**drag-and-drop between nodes**. The widget tracks drag gestures
internally and emits a `DragCompleted { sources, destination }`
event on successful drop; the application decides what to do with
the paths (move / copy / symlink / upload / anything). The widget
performs no filesystem operations itself.

### Added

- **New event variants on `DirectoryTreeEvent`:**
  - `Drag(DragMsg)` — opaque drag-machinery event. Apps route it
    back through `tree.update()` unchanged, exactly like `Loaded`.
  - `DragCompleted { sources: Vec<PathBuf>, destination: PathBuf }`
    — fires when the user releases the mouse over a valid folder
    row. Apps observe this event to perform the actual filesystem
    operation.
- **New public `DragMsg` enum** — re-exported from the crate root.
  Variants: `Pressed(PathBuf, bool)`, `Entered(PathBuf)`,
  `Exited(PathBuf)`, `Released(PathBuf)`, `Cancelled`. Generally
  constructed by the widget itself; apps only need to match on
  `Cancelled` if they want to force-cancel a drag.
- **Three new accessors on `DirectoryTree`:**
  - `is_dragging() -> bool`
  - `drop_target() -> Option<&Path>` — the currently-hovered valid
    folder target, or `None` when over empty space / files / self / a
    descendant of a source.
  - `drag_sources() -> &[PathBuf]` — paths being dragged.
- **Drag-aware drop-target highlight** in the built-in view. The
  hovered folder row paints with the theme's `success.weak`
  background and a `success.strong` outline.
- **Deferred-selection pattern.** Mouse-down on a row no longer
  immediately collapses a multi-selection down to that row. If the
  user releases on the same row the widget emits a delayed
  `Selected(path, is_dir, Replace)`; if they release on a different
  valid folder it emits `DragCompleted` instead. This matches
  Explorer / Finder behaviour — you can drag a multi-selection
  without losing it.
- **Multi-item drag.** Pressing on a row that's already in the
  selection drags the whole selected set; pressing on an unselected
  row drags only that row.
- **`Escape` cancels an in-flight drag.** The widget's built-in key
  handler produces `Drag(Cancelled)` when `Escape` is pressed while
  a drag is active. When no drag is active, `Escape` stays unbound
  so apps can still use it for their own UI.
- **`examples/drag_drop.rs`** — a complete working example that
  performs `fs::rename` on `DragCompleted` and refreshes affected
  folders. Includes modifier tracking for multi-select, live
  drag-preview status bar, and a safe default scratch directory
  under the OS temp dir so you can experiment without data risk.

### Changed

- **Row hit-testing is now a `mouse_area` around a styled
  `container`** rather than a `button`. This was required to
  observe mouse-down (for drag start) separately from mouse-up
  (for click vs. drag disambiguation) — iced 0.14's
  `button::on_press` fires only on click-completion. The
  user-visible row appearance is unchanged for the normal and
  selected states (the container style reproduces `button::text`
  and `button::primary` via the theme's `palette.primary.base`).
  Files still align with folders because the caret is still its
  own button on directory rows and a fixed-size `Space` on files.
- **Known trade-off:** non-selected rows lose the native hover
  glow that `button::text` provided, because `mouse_area` + plain
  `container` don't track hover state. Selected rows still paint
  with the primary-base background. A future version may add an
  explicit hover style; the omission is deliberate for now in
  favour of shipping drag-and-drop soon.
- **Known behaviour:** if the user releases the mouse outside the
  tree (over a scrollbar, empty space, another window), the drag
  state stays active until they press `Escape` or the app forces
  a `Cancelled`. A fix via an `iced::event::listen` subscription
  is possible but deliberately deferred — this matches what most
  native file browsers do.

### Public API — no breaking changes to existing code paths

- `DirectoryTreeEvent` is non-exhaustive and gains two new variants.
  Apps that exhaustively `match` on it without a `_` arm need to
  add arms for `Drag` (route back to `tree.update`) and
  `DragCompleted` (the app's own move/copy/etc. logic). Most apps
  use `.map(MyMessage::Tree)` and don't need changes.

### Tests

- **100 total (up from 70):** 60 unit + 10 + 29 integration + 1
  doctest. New coverage includes 6 `DragState::is_valid_target`
  unit tests (file rejected, self rejected, descendant rejected,
  sibling accepted, parent accepted, prefix-but-not-ancestor
  accepted), 12 state-machine transition tests for `on_drag`, 2
  keyboard tests for Escape-cancels / Escape-unbound-without-drag,
  and 11 integration tests driving the full public API against a
  real temp filesystem.

## [0.3.0] — 2026-04-23

Delivers the first of the five v1.0-required roadmap items:
**multi-select** (Shift/Ctrl-click, Shift-arrow, Space-toggle).

### Added

- **Multi-select via [`SelectionMode`].** New public enum with three
  variants — `Replace` (default / plain click), `Toggle`
  (Ctrl/Cmd-click), and `ExtendRange` (Shift-click). A
  `from_modifiers(Modifiers)` convenience maps an iced `Modifiers`
  value to the right mode.
- **Three new accessors on `DirectoryTree`:**
  - `selected_paths() -> &[PathBuf]` — the full selected set.
  - `anchor_path() -> Option<&Path>` — the pivot for `ExtendRange`
    (not moved by `ExtendRange` itself, matching Explorer/Finder).
  - `is_selected(&Path) -> bool` — membership check.
- **Keyboard multi-select:**
  - `Shift + ↑/↓/Home/End` extend the selected range.
  - `Space` and `Ctrl+Space` toggle the active path in/out of the
    set (changed from v0.2 — see BREAKING below).
- **`examples/multi_select.rs`** — full working demo showing the
  modifier-tracking pattern and a live multi-selection status bar.

### Changed — BREAKING

- `DirectoryTreeEvent::Selected(PathBuf, bool)` →
  `DirectoryTreeEvent::Selected(PathBuf, bool, SelectionMode)`. Existing
  apps only need to add `SelectionMode::Replace` (or `_` in
  pattern-matches where mode is irrelevant). Migration is a one-line
  sed per match site.
- Internal state: `selected_path: Option<PathBuf>` is replaced by three
  fields (`selected_paths: Vec<PathBuf>`, `active_path: Option<PathBuf>`,
  `anchor_path: Option<PathBuf>`). The public `selected_path()` accessor
  still returns the last-touched path, preserving v0.2 semantics for
  single-select callers — no change required for apps that don't
  care about multi-select.
- `handle_key` now uses the `modifiers` argument (ignored in v0.2).
  `Space` is now `Toggle` instead of "re-emit current selection" — the
  v0.2 behaviour was rarely useful and the new one matches VS Code,
  Finder, and Explorer.

### View-level click behaviour

The built-in view emits `Selected(path, is_dir, SelectionMode::Replace)`
on every click because iced 0.14's `button::on_press` callback cannot
observe modifier keys. Applications that want multi-select track
modifier state themselves via a `keyboard::listen()` subscription and
rewrite the mode before forwarding — see `examples/multi_select.rs`.
This will become unnecessary if a future iced release exposes
modifiers at press time.

### Test coverage

- 70 tests pass (up from 52 in v0.2):
  - 41 unit tests (was 27): + 5 `SelectionMode`, + 3 shift/ctrl
    keyboard binding tests, + 6 multi-select state-machine tests.
  - 10 integration tests (unchanged count; all migrated to the
    3-arg `Selected` form).
  - 18 tree-layer tests (was 14): + 4 new v0.3 multi-select
    integration tests over a real filesystem (toggle builds up a
    set, range covers siblings, filter change preserves every
    selected path, `selected_path()` tracks the last action target).
  - 1 compile-only doctest.

## [0.2.0] — 2026-04-23

The v0.2 release knocks out every item on the v0.2 — v0.3 roadmap
and bumps the `swdir` dependency from 0.9 to 0.10.

### Added

- **Per-path selection tracking.** Selection is now stored as a
  `selected_path: Option<PathBuf>` on `DirectoryTree`, with the
  per-node `is_selected` flag re-synced by the update layer. The
  cursor survives filter changes and subtree re-loads, and `set_filter`
  re-applies selection against the rebuilt node graph automatically.
- **Expansion state also survives filter changes.** `set_filter`'s
  rebuild now carries whole child subtrees over path-keyed instead
  of reconstructing empty nodes — an opened folder stays open, an
  already-loaded descendant stays loaded.
- **Keyboard navigation.** New `DirectoryTree::handle_key` adapter
  that turns an `iced::keyboard::Key` + `Modifiers` pair into an
  appropriate `DirectoryTreeEvent`. Bindings: `↑`/`↓` move along
  visible rows, `Home`/`End` jump to first/last, `Enter` toggles
  folders, `Space` re-emits the current selection, `←` collapses
  or moves to parent, `→` expands or moves to first child. The
  widget stays focus-neutral — the app decides when the tree
  "has focus" and pipes keypresses in.
- **Pluggable scan executor.** New `ScanExecutor` trait with
  `ScanJob` / `ScanFuture` type aliases and a default
  `ThreadExecutor` implementation. Apps that already have a
  blocking-task pool (tokio, smol, rayon, ...) can implement the
  trait and plug in via `DirectoryTree::with_executor(Arc<dyn
  ScanExecutor>)`. Default behaviour is unchanged — one
  `std::thread::spawn` per expansion via `ThreadExecutor`.
- New `examples/keyboard_nav.rs` demonstrating the subscription
  pattern.

### Changed

- **Dependency**: `swdir = "0.9"` → `swdir = "0.10"`. iced-swdir-tree
  only uses `swdir::scan_dir`, `DirEntry`, and `ScanError`, all of
  which are unchanged across swdir's 0.10 cleanup release — no API
  impact on downstream apps.
- `TreeNode::find_selected` removed (was crate-private; selection is
  now cursor-based, so the whole-tree walk is no longer needed).

### Resolved

- The v0.1 known limitation "filter change drops per-node
  selection state" is gone. Selection and expansion both survive
  runtime filter changes now.

### Test coverage

- 52 tests pass (up from 25 in v0.1):
  - 27 unit tests (14 original + 13 for the new keyboard module).
  - 10 integration tests (one updated for the new selection-preserving
    semantics).
  - 14 tree-layer tests (10 original + 2 rewritten for v0.2
    semantics + 4 new v0.2-specific coverage of executor swap and
    subtree-preservation-across-filter-change).
  - 1 compile-only doctest.

## [0.1.0] — Unreleased

Initial release.

### Added

- `DirectoryTree` widget with lazy, asynchronous loading via
  `iced::Task::perform` + `swdir::scan_dir`.
- `DirectoryFilter` with three modes: `FoldersOnly`,
  `FilesAndFolders` (default), `AllIncludingHidden`.
- `DirectoryTreeEvent` with `Toggled`, `Selected`, and opaque
  `Loaded` variants.
- `TreeNode`, `LoadedEntry`, `TreeCache` state types.
- `TreeConfig` for per-tree configuration (root, filter, depth limit).
- Crate-level `Error` type (clone-able; wraps `io::ErrorKind` + message).
- Builder API: `new`, `with_filter`, `with_max_depth`, `set_filter`.
- Read accessors: `filter`, `max_depth`, `root_path`, `selected_path`.
- `icons` feature flag, gating a lucide-icons-based glyph renderer.
  Public API is identical with and without the feature.
- Per-OS hidden-file detection: dotfile on Unix, `HIDDEN` attribute with
  dotfile fallback on Windows, dotfile elsewhere.
- Stale-result detection via a per-scan generation counter.
- Permission-denied and missing-path handling: surfaced as
  `TreeNode::error`, greyed out in the view, never panicking.
- Sorted output: directories first, files second, each group sorted
  case-insensitively.
- 25 tests (14 unit + 10 integration + 1 compile-only doc-test)
  covering every filter mode, expand/collapse round-trips, selection,
  stale-result rejection, permission-denied, and nonexistent paths.
- Two examples: `basic` (Unicode-symbol fallback) and `with_icons`
  (lucide-icons feature).

### Known limitations (at v0.1; resolved in v0.2)

- **Filter change drops per-node selection state.** *Fixed in v0.2 —
  selection is now stored by path on the widget, not by flag on
  rebuilt nodes.*
- **Multi-select, drag-and-drop, and search are not implemented.** See
  ROADMAP.
- **View-layer virtualization is delegated to iced's `Scrollable`.**
  Very large trees (hundreds of thousands of loaded nodes) may show
  layout-pass slowdowns. A future custom low-level widget could narrow
  the rendered slice to only the on-screen rows.
