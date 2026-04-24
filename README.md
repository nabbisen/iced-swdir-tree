# iced-swdir-tree

[![crates.io](https://img.shields.io/crates/v/iced-swdir-tree?label=rust)](https://crates.io/crates/iced-swdir-tree)
[![License](https://img.shields.io/github/license/nabbisen/iced-swdir-tree)](https://github.com/nabbisen/iced-swdir-tree/blob/main/LICENSE)
[![Documentation](https://docs.rs/iced-swdir-tree/badge.svg?version=latest)](https://docs.rs/iced-swdir-tree)
[![Dependency Status](https://deps.rs/crate/iced-swdir-tree/latest/status.svg)](https://deps.rs/crate/iced-swdir-tree)

> A batteries-included directory-tree widget for [iced](https://iced.rs) —
> lazy-loading, async, multi-select, drag-and-drop, live search.

## Overview

A reusable iced widget for displaying a directory tree with
selection, lazy loading, filtering, and asynchronous traversal.
Built on [swdir](https://crates.io/crates/swdir)'s `scan_dir` for
single-level, non-recursive directory listings — ideal for GUI
trees that expand one folder at a time.

The widget never blocks the UI thread on disk I/O; it never
touches the filesystem beyond reading directory listings; and it
ships its full event surface (selection, drag-drop, keyboard,
search) behind a small, typed API that composes with the iced
`Task` / `Subscription` model.

## When to use it

Reach for this crate when your iced app needs **any** of:

- A file/folder picker with multi-select.
- A project-navigator pane (code editor, asset browser,
  file-manager side panel).
- Drag-and-drop between folders — you react to a
  `DragCompleted { sources, destination }` event and perform the
  move/copy/upload yourself.
- A searchable directory view with real-time type-ahead
  filtering.

If you only need a one-shot "pick a file" dialog, the OS-native
file-chooser is almost certainly a better fit.

## Quick start

```toml
[dependencies]
iced = "0.14"
iced-swdir-tree = "0.6"
```

```rust,no_run
use std::path::PathBuf;
use iced::{Element, Task};
use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent};

#[derive(Debug, Clone)]
enum Message {
    Tree(DirectoryTreeEvent),
}

struct App {
    tree: DirectoryTree,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let tree = DirectoryTree::new(PathBuf::from("."))
            .with_filter(DirectoryFilter::FilesAndFolders);
        (Self { tree }, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tree(event) => self.tree.update(event).map(Message::Tree),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        self.tree.view(Message::Tree)
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view).run()
}
```

For real lucide glyphs instead of the Unicode-symbol fallback,
enable the `icons` feature and register the bundled font:

```toml
iced-swdir-tree = { version = "0.6", features = ["icons"] }
```

```rust,ignore
iced::application(App::new, App::update, App::view)
    .font(iced_swdir_tree::LUCIDE_FONT_BYTES)
    .run()
```

Working apps live in [`examples/`](examples/): `keyboard_nav`,
`multi_select`, `drag_drop`, `search`. Run them with
`cargo run --example <name>`.

## Design notes

- **Lazy, async, cache-backed.** Only the root is eagerly
  created. Each expansion dispatches a scan through a pluggable
  [`ScanExecutor`](docs/guide/custom-executor.md) and merges the result back
  into a generation-tagged cache. Filter changes and search
  re-derive from the cache — no re-scan.
- **Every feature is orthogonal.** Selection survives filter
  flips, subtree reloads, collapse/re-expand cycles, and
  search-hidden rows. Drag state is separate from selection
  state. Search doesn't mutate expansion. These invariants are
  tested (the crate ships 140+ tests).
- **The widget owns UI state, not filesystem state.** It never
  renames, deletes, moves, or writes. Drag-and-drop produces a
  `DragCompleted` event for your app to handle — the widget's
  job ends at "here's what the user asked for."
- **Safety valves where defaults matter.** Prefetch
  (`with_prefetch_limit`) won't enter `.git`, `node_modules`,
  `target`, or other common "don't scan this" directories out
  of the box; the skip list is
  [configurable](docs/guide/prefetch.md#safety-valve-the-skip-list).
  `max_depth` caps recursion. Generation tags drop stale scan
  results that returned after a collapse.

## Documentation

**📚 Full reference in [docs/](docs/)**, organized by intent:

- **Guide** (build something) — [Configuration](docs/guide/configuration.md)
  · [Multi-select](docs/guide/multi-select.md)
  · [Drag-and-drop](docs/guide/drag-and-drop.md)
  · [Keyboard navigation](docs/guide/keyboard-navigation.md)
  · [Incremental search](docs/guide/incremental-search.md)
  · [Parallel pre-expansion](docs/guide/prefetch.md)
  · [Custom scan executor](docs/guide/custom-executor.md)
- **Reference** (look something up) — [Features](docs/reference/features.md)
  · [Events](docs/reference/events.md)
- **Internals** (understand or contribute) — [Architecture](docs/internals/architecture.md)
  · [Development & testing](docs/internals/development.md)
- **Release notes** — [CHANGELOG](CHANGELOG.md) · [ROADMAP](ROADMAP.md)
