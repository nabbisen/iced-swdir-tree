# iced-swdir-tree

[![crates.io](https://img.shields.io/crates/v/iced-swdir-tree?label=rust)](https://crates.io/crates/iced-swdir-tree)
[![License](https://img.shields.io/github/license/nabbisen/iced-swdir-tree)](https://github.com/nabbisen/iced-swdir-tree/blob/main/LICENSE)
[![Documentation](https://docs.rs/iced-swdir-tree/badge.svg?version=latest)](https://docs.rs/iced-swdir-tree)
[![Dependency Status](https://deps.rs/crate/iced-swdir-tree/latest/status.svg)](https://deps.rs/crate/iced-swdir-tree)

A reusable [iced](https://iced.rs) widget for displaying a directory tree with
selection, lazy loading, filtering and asynchronous traversal.

Built on top of [swdir](https://crates.io/crates/swdir)'s `scan_dir` for
single-level, non-recursive directory listings — ideal for GUI trees that
expand one folder at a time.

## Features

- **Multi-select** with Shift/Ctrl-click and Shift-arrow range extension.
  A per-path authoritative set survives filter changes and subtree
  reloads; see [Multi-select](#multi-select).
- **Drag-and-drop between nodes.** Drag one or more selected paths
  onto another folder; the widget emits a `DragCompleted { sources,
  destination }` event and the app performs the actual move/copy/
  upload/whatever. The widget performs no filesystem operations
  itself. See [Drag-and-drop](#drag-and-drop).
- **Parallel pre-expansion.** Opt into `with_prefetch_limit(N)` and
  the widget will speculatively scan the first `N` folder-children
  of any folder the user expands, in parallel via the executor, so
  clicking any of them is instant. One level deep only (no cascade).
  See [Parallel pre-expansion](#parallel-pre-expansion).
- **Incremental search.** `tree.set_search_query(q)` narrows the
  visible rows to basename-substring matches plus their ancestor
  chain, so users see tree context alongside their hits. Selection
  survives the filter. See [Incremental search](#incremental-search).
- **Lazy loading.** Only the root is created eagerly; child folders are
  scanned on first expand.
- **Non-blocking.** Directory traversal runs on a worker thread through
  `iced::Task::perform`; the UI thread never stalls on disk I/O.
  Plug in your own executor (`tokio`, `smol`, etc.) via
  [`with_executor`](#custom-scan-executor) if you don't want the
  per-expansion thread-spawn default.
- **Three display filters.** `FoldersOnly`, `FilesAndFolders` (default),
  `AllIncludingHidden`. Filter changes are applied from an in-memory
  cache, so switching is instant — no re-scan. Expansion state and
  selection survive the swap.
- **Keyboard navigation.** Arrow keys, `Home`/`End`, `Enter`,
  `Space`, `←`/`→`, plus Shift-modified variants for range
  extension and `Escape` to cancel a drag — see
  [Keyboard navigation](#keyboard-navigation).
- **Stale-result handling.** Every scan carries a generation counter, so a
  collapse/re-expand cycle safely discards in-flight results from the
  cancelled round-trip.
- **Error tolerance.** Permission denials, missing paths, and symlink
  cycles are surfaced as per-node errors that the view greys out — no
  panics, no UI freezes.
- **Optional lucide icons.** Disabled by default; enable the `icons`
  feature to pull in real vector glyphs. The public API is identical in
  both modes.
- **Cross-platform.** Hidden-file detection follows OS conventions: dotfile
  on Unix, `HIDDEN` attribute plus dotfile fallback on Windows, dotfile
  elsewhere.

## Installation

```toml
[dependencies]
iced = "0.14"
iced-swdir-tree = "0.6"
```

To use real lucide icons instead of the Unicode-symbol fallback:

```toml
[dependencies]
iced = "0.14"
iced-swdir-tree = { version = "0.6", features = ["icons"] }
```

The crate works without your application adding `swdir` directly — the
widget internally wraps it and exposes the pieces you need through its own
API.

## Quick start

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
            Message::Tree(event) => {
                // React to app-level side effects BEFORE forwarding.
                // The third tuple element is the `SelectionMode`; match
                // with `_` when you only care about the clicked path.
                if let DirectoryTreeEvent::Selected(path, is_dir, _) = &event {
                    println!("selected {:?} (dir={})", path, is_dir);
                }
                self.tree.update(event).map(Message::Tree)
            }
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

For a working example with a filter picker and a selection status bar, see
[`examples/basic.rs`](examples/basic.rs). For the lucide-icons version, see
[`examples/with_icons.rs`](examples/with_icons.rs).

### Using the `icons` feature

When `icons` is enabled, register the bundled lucide TTF with iced at
startup:

```rust,ignore
use iced_swdir_tree::LUCIDE_FONT_BYTES;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .font(LUCIDE_FONT_BYTES)
        .run()
}
```

Without this registration the icon widgets still render, but as tofu
squares — the default system font doesn't have the lucide glyphs.

## Configuration

```rust,no_run
# use std::path::PathBuf;
# use iced_swdir_tree::{DirectoryFilter, DirectoryTree};
let tree = DirectoryTree::new(PathBuf::from("."))
    .with_filter(DirectoryFilter::AllIncludingHidden)
    .with_max_depth(5);
```

| Method | Purpose |
|---|---|
| `new(root)` | Build a tree rooted at `root`. Only the root is eagerly created. |
| `with_filter(f)` | Builder form of `set_filter`. |
| `with_max_depth(d)` | Refuse to load below depth `d` (0 = root children only). |
| `with_executor(e)` | Route blocking scans through a custom [`ScanExecutor`](#custom-scan-executor). |
| `set_filter(f)` | Change the filter at runtime. Re-derives from cache; no I/O. |
| `handle_key(k, m)` | Translate a keyboard event into a `DirectoryTreeEvent` — see [Keyboard navigation](#keyboard-navigation). |
| `filter()`, `max_depth()`, `root_path()` | Config accessors. |
| `selected_path()` | Most recently touched path (`v0.2` single-select accessor). |
| `selected_paths()` | The full selected set (`v0.3` multi-select). |
| `anchor_path()` | Pivot for `SelectionMode::ExtendRange`. |
| `is_selected(path)` | Membership check. |

## Events

The widget emits `DirectoryTreeEvent`:

- `Toggled(PathBuf)` — the user clicked the caret on a folder.
- `Selected(PathBuf, bool, SelectionMode)` — the user selected a
  row; `bool` is `true` for directories, `false` for files, and
  the [`SelectionMode`](#multi-select) controls how the event
  composes with any existing selection.
- `Loaded(LoadPayload)` — internal; a pending scan completed.
  Parent applications route it straight back into `update()` without
  inspecting it.

## Multi-select

The widget keeps a full selected set, a "most-recent-action" active
path, and an anchor path for Shift-range extension.
[`SelectionMode`] — exported from the crate root — controls how each
click composes:

| Mode | Effect |
|---|---|
| `Replace` | Clear the set; the new path becomes the only selection. Updates both active and anchor. |
| `Toggle`  | Add if absent, remove if present. Updates both active and anchor. |
| `ExtendRange` | Replace the set with the visible rows between anchor and target, inclusive. Only active moves. Falls back to `Replace` if no anchor is set. |

### View-level click behaviour

iced 0.14's `button::on_press` cannot observe modifier keys, so the
widget's built-in view always emits `SelectionMode::Replace` on
click. Applications that want real multi-select track modifier
state separately and rewrite the event in their own update handler:

```rust,ignore
use iced::keyboard::{self, Modifiers};
use iced_swdir_tree::{DirectoryTreeEvent, SelectionMode};

// In your update:
Message::Tree(DirectoryTreeEvent::Selected(path, is_dir, _)) => {
    let mode = SelectionMode::from_modifiers(self.modifiers);
    let event = DirectoryTreeEvent::Selected(path, is_dir, mode);
    self.tree.update(event).map(Message::Tree)
}
Message::ModifiersChanged(m) => {
    self.modifiers = m;
    Task::none()
}

// In your subscription:
fn subscription(app: &App) -> iced::Subscription<Message> {
    keyboard::listen().map(|event| match event {
        keyboard::Event::ModifiersChanged(m) => Message::ModifiersChanged(m),
        keyboard::Event::KeyPressed { key, modifiers, .. } =>
            Message::TreeKey(key, modifiers),
        _ => /* ... */,
    })
}
```

See [`examples/multi_select.rs`](examples/multi_select.rs) for a
complete working app with a live selection-count status bar.

## Drag-and-drop

The widget tracks drag gestures internally and emits a
`DragCompleted { sources, destination }` event when the user
releases over a valid folder. The widget **does not** touch the
filesystem — your app reacts to `DragCompleted` and performs the
actual move / copy / upload / whatever, then re-scans affected
folders so the view reflects the new layout.

```rust,ignore
match message {
    Message::Tree(DirectoryTreeEvent::DragCompleted {
        sources,
        destination,
    }) => {
        for src in &sources {
            if let Some(name) = src.file_name() {
                let _ = std::fs::rename(src, destination.join(name));
            }
        }
        // Refresh the destination and each source's parent so the
        // tree picks up the new layout. A collapse+re-expand via
        // two Toggled events is the simplest invalidation.
        let mut tasks = vec![];
        let mut refresh: std::collections::HashSet<PathBuf> = Default::default();
        refresh.insert(destination);
        for s in &sources {
            if let Some(p) = s.parent() { refresh.insert(p.into()); }
        }
        for p in refresh {
            tasks.push(Task::done(Message::Tree(DirectoryTreeEvent::Toggled(p.clone()))));
            tasks.push(Task::done(Message::Tree(DirectoryTreeEvent::Toggled(p))));
        }
        Task::batch(tasks)
    }
    Message::Tree(event) => self.tree.update(event).map(Message::Tree),
    // ...
}
```

Pressing the mouse on a row that's already in the selection drags
the whole selected set; pressing on an unselected row drags only
that row. `Escape` cancels an in-flight drag. If the mouse is
released outside the tree (or over empty space), the drag stays
active until `Escape` or an app-initiated cancel — deliberately
chosen to match native file-browser behaviour.

Three read-only accessors let your UI reflect drag state:

```rust,ignore
tree.is_dragging();      // bool
tree.drag_sources();     // &[PathBuf]
tree.drop_target();      // Option<&Path> — hovered valid folder
```

See [`examples/drag_drop.rs`](examples/drag_drop.rs) for a complete
working app with `fs::rename` on drop, post-move refresh, and a
live drag-preview status bar.

## Parallel pre-expansion

Apps on a fast executor (tokio, smol, rayon) usually have more I/O
capacity than one-folder-per-gesture uses. `with_prefetch_limit(N)`
opts into parallel pre-expansion: whenever a user expands a folder
and its children come back, the widget speculatively fires scan
tasks for the first `N` of those children that are folders. Those
scans populate the cache but do **not** auto-expand anything —
`is_loaded = true` without `is_expanded = true`. When the user
later clicks to expand one of the pre-fetched folders, no I/O
happens: it's an instant fast-path re-expand.

```rust,ignore
use iced_swdir_tree::DirectoryTree;
use std::sync::Arc;

let tree = DirectoryTree::new(root)
    .with_executor(Arc::new(MyTokioExecutor))
    .with_prefetch_limit(10);
```

Pass `0` (or don't call `with_prefetch_limit` at all) to disable
prefetch — that's the default and matches v0.1–0.4 behaviour
exactly. Prefetch is **one level deep**: a folder that loaded via
prefetch does not itself trigger further prefetches, so the I/O
budget is `per_parent` scans per user expansion, not
`per_parent ^ depth`. It also respects `with_max_depth(..)`:
children past the cap are skipped rather than scanned.

Sensible values depend on your executor. On the default
`ThreadExecutor` (one `std::thread::spawn` per scan), keep it
modest (5–25) — each prefetch becomes a real OS thread. On a
bounded tokio/smol pool, a higher value is free: excess tasks just
queue behind the pool's worker cap.

## Incremental search

Wire an `iced::widget::text_input` into the tree via
`DirectoryTree::set_search_query`. The widget narrows its visible
rows to basename-substring matches (case-insensitive) plus every
ancestor of every match — so users see where matches live in the
tree, not just isolated filenames.

```rust,ignore
#[derive(Debug, Clone)]
enum Message {
    SearchChanged(String),
    // ...
}

// In update:
Message::SearchChanged(q) => {
    self.tree.set_search_query(q);
    Task::none()
}

// In view:
text_input("Filter...", &self.query).on_input(Message::SearchChanged)
```

Four accessors drive a "N matches" status line or a clear button:

```rust,ignore
tree.is_searching();             // bool
tree.search_query();             // Option<&str> (original casing)
tree.search_match_count();       // usize — excludes ancestor rows
tree.clear_search();             // drop the query
```

Semantics:

- **Case-insensitive basename substring match.** The path
  components ("/src/…") don't match — only the filename at each
  level does.
- **Empty string = cleared search.** There is no "searching for
  an empty string" state.
- **Already-loaded nodes only.** Matches inside unloaded folders
  don't appear until the folder loads. Combine with
  `with_prefetch_limit(N)` for broader coverage without the user
  expanding everything manually.
- **Sees through collapsed-but-loaded folders.** A match deep
  inside a collapsed subtree still shows up; ancestors render as
  if expanded.
- **Selection survives.** Hidden-by-search selections are
  preserved and reappear when the query clears.

One documented limitation: clicking a folder during search does
NOT escape the filter. The view stays narrowed to matches plus
ancestors. To explore outside the match set, clear the search
first. See [`examples/search.rs`](examples/search.rs) for a
complete working app with text-input, counter, and expand-all
button.

## Keyboard navigation

`DirectoryTree::handle_key(&Key, Modifiers) -> Option<DirectoryTreeEvent>`
translates a key press into the right event. The widget stays
focus-neutral — you decide when the tree has focus and subscribe
to the key stream yourself:

```rust,ignore
use iced::keyboard;

fn subscription(app: &App) -> iced::Subscription<Message> {
    keyboard::listen().map(|event| match event {
        keyboard::Event::KeyPressed { key, modifiers, .. } =>
            Message::TreeKey(key, modifiers),
        _ => Message::Noop,
    })
}

// ...in update:
Message::TreeKey(key, mods) => {
    if let Some(event) = self.tree.handle_key(&key, mods) {
        return self.tree.update(event).map(Message::Tree);
    }
    Task::none()
}
```

| Key | Behaviour |
|---|---|
| `↑` / `↓` | Move selection to previous / next visible row. |
| `Shift` + `↑` / `↓` | Extend the selected range toward the previous / next row. |
| `Home` / `End` | Jump to first / last visible row. |
| `Shift` + `Home` / `End` | Extend the range to the first / last row. |
| `Enter` | Toggle the selected directory (no-op on files). |
| `Space` / `Ctrl` + `Space` | Toggle the active path in or out of the selected set. |
| `←` | Collapse selected directory, or move to parent. |
| `→` | Expand selected directory, or move to first child. |
| `Esc` | Cancel an in-flight drag (only bound during drag, so apps can still use `Esc` for their own UI otherwise). |

See [`examples/keyboard_nav.rs`](examples/keyboard_nav.rs) for a
single-select navigation demo and
[`examples/multi_select.rs`](examples/multi_select.rs) for
multi-select with Shift/Ctrl-click.

## Custom scan executor

By default the widget spawns one `std::thread` per folder expansion
via `ThreadExecutor`. Apps that already run a blocking-task pool
(tokio, smol, rayon, ...) can route through it by implementing
`ScanExecutor`:

```rust,ignore
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use iced_swdir_tree::{ScanExecutor, ScanJob, ScanFuture, DirectoryTree};

struct TokioExecutor;

impl ScanExecutor for TokioExecutor {
    fn spawn_blocking(&self, job: ScanJob) -> ScanFuture {
        Box::pin(async move {
            tokio::task::spawn_blocking(job)
                .await
                .expect("scan task panicked")
        })
    }
}

let tree = DirectoryTree::new(root)
    .with_executor(Arc::new(TokioExecutor));
```

The default behaviour is unchanged if you don't call
`with_executor` — existing v0.1 code keeps working as-is.

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md).

## Testing

See [DEVELOPMENT.md#testing](DEVELOPMENT.md#testing).
