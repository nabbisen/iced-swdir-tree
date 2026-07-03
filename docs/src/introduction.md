# iced-swdir-tree

Tree widgets for the [iced](https://iced.rs) GUI framework.

The crate ships two widgets that share the same navigation model —
selection, keyboard control, incremental search, drag-and-drop, and
pluggable icon themes:

- **`DirectoryTree`** — a lazy-loading, async, cache-backed view of a
  real filesystem directory, built on
  [swdir](https://crates.io/crates/swdir)'s `scan_dir`. It expands one
  folder at a time and never blocks the UI thread on disk I/O.
- **`ItemTree<T>`** — a synchronous, in-memory tree over your own data
  `T`, addressed by stable `NodeId`s. Feed it a tree, get key-based
  diffing that preserves expansion and selection across edits.

The widgets own **UI state only**. They never rename, delete, move, or
write; drag-and-drop reports the user's intent as an event and your
application performs the change.

## Finding your way around

This documentation is organized by what you are trying to do.

**Building an app?** The [User Guide](guide/getting-started.md) is
task-oriented, with code you can paste in — start at
[Getting started](guide/getting-started.md), then dip into the topic
you need. New to the crate? The [FAQ](guide/faq.md) answers the common
first questions.

**Looking a fact up?** The Reference covers the
[feature list](reference/features.md), the
[event enums](reference/events.md) your `update` handler consumes, and
the numbered [feature specifications](internals/feature-specs.md) that
define exact behaviour.

**Understanding or contributing?** The Maintainers & Contributors
section covers the [design principles](internals/core-design.md),
[architecture](internals/architecture.md),
[data model](internals/data-model.md),
[state machine](internals/state-machine.md), how to
[port the design to another framework](internals/porting-to-dioxus.md),
and how to [run the test matrix](internals/development.md) locally.

## Project links

- Source, issues, and releases: the crate's
  [GitHub repository](https://github.com/nabbisen/iced-swdir-tree).
- API docs (rustdoc): [docs.rs](https://docs.rs/iced-swdir-tree).
- Release history: [CHANGELOG](../../CHANGELOG.md).
- What's shipped and planned: [ROADMAP](../../ROADMAP.md).
