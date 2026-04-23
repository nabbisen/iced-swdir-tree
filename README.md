# iced-swdir-tree

A reusable [iced](https://iced.rs) widget for displaying a directory tree with
selection, lazy loading, filtering and asynchronous traversal.

Built on top of [swdir](https://crates.io/crates/swdir)'s `scan_dir` for
single-level, non-recursive directory listings — ideal for GUI trees that
expand one folder at a time.

## Features

- **Single-select** with per-path persistence. Clicks emit a
  `Selected(PathBuf, bool)` event; the cursor survives filter
  changes and subtree reloads.
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
  `Space`, `←`/`→` — see [Keyboard navigation](#keyboard-navigation).
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
iced-swdir-tree = "0.2"
```

To use real lucide icons instead of the Unicode-symbol fallback:

```toml
[dependencies]
iced = "0.14"
iced-swdir-tree = { version = "0.2", features = ["icons"] }
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
                if let DirectoryTreeEvent::Selected(path, is_dir) = &event {
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
| `filter()`, `max_depth()`, `root_path()`, `selected_path()` | Read accessors. |

## Events

The widget emits `DirectoryTreeEvent`:

- `Toggled(PathBuf)` — the user clicked the caret on a folder.
- `Selected(PathBuf, bool)` — the user clicked a row; `bool` is
  `true` for directories, `false` for files.
- `Loaded(LoadPayload)` — internal; a pending scan completed.
  Parent applications route it straight back into `update()` without
  inspecting it.

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
| `Home` / `End` | Jump to first / last visible row. |
| `Enter` | Toggle the selected directory (no-op on files). |
| `Space` | Re-emit current selection as a `Selected` event. |
| `←` | Collapse selected directory, or move to parent. |
| `→` | Expand selected directory, or move to first child. |

See [`examples/keyboard_nav.rs`](examples/keyboard_nav.rs) for a
complete working app.

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
