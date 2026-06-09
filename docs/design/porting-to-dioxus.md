# Porting to Dioxus

This document maps every significant design concern in
`iced-swdir-tree` to its Dioxus equivalent. It assumes
familiarity with Dioxus hooks, signals, and coroutines
(Dioxus 0.5+).

---

## Conceptual mapping

| iced concept | Dioxus equivalent |
| --- | --- |
| `App { tree: DirectoryTree }` | `use_signal(|| DirectoryTree::new(root))` |
| `Message::Tree(ev)` + `update()` | Event handler closures; signal mutations |
| `Task<Message>` | `spawn(async { … })` or a `use_coroutine` channel |
| `tree.view(Message::Tree)` | `rsx!` that reads from signals and calls `tree.view()` |
| `iced::subscription(keyboard)` | `use_coroutine` or `dioxus_hooks::use_future` reading a JS event listener |
| `tree.handle_key(key, mods)` | Called inside an `onkeydown` handler |
| `element.on_press` / `on_release` | `onmousedown` / `onmouseup` |
| `element.on_enter` / `on_exit` | `onmouseenter` / `onmouseleave` |
| `Length::Fill` | CSS `width: 100%; flex: 1` |
| `scrollable(list)` | `div { overflow-y: auto; height: 100% }` |
| `iced::Font::with_name("lucide")` | CSS `font-family: "lucide"` on the glyph span |

---

## Overall architecture

In iced, `DirectoryTree` is owned by the application struct.
State mutations happen synchronously inside `update()`.
Side-effects (scans) are returned as `Task<Message>` values
and driven by the iced runtime.

In Dioxus, the simplest faithful port wraps the widget state
in a single signal:

```rust
// Component root
let tree = use_signal(|| {
    DirectoryTree::new(root_path.clone())
        .with_filter(DirectoryFilter::FilesAndFolders)
});
```

State mutations write to the signal; the framework diffs the
virtual DOM. Scans are issued through a coroutine channel.

### Alternative: split signals

For finer-grained re-renders you can split state:

```rust
let nodes     = use_signal(|| TreeNodes::new(root));
let selection = use_signal(|| SelectionState::default());
let search    = use_signal(|| SearchState::default());
let drag      = use_signal(|| DragState::default());
```

This is an optimisation — start with the monolithic signal.

---

## Async scanning

### iced approach

```
iced::Task::perform(
    async move { tokio::task::spawn_blocking(|| swdir::scan_dir(path)).await },
    |result| Message::Tree(DirectoryTreeEvent::Loaded(payload))
)
```

The returned `Task` is handed to the iced runtime; the callback
fires on the main thread when the future resolves.

### Dioxus approach

Dioxus provides `use_coroutine` for persistent background
loops and `spawn` for one-shot tasks:

```rust
// Set up a coroutine to process incoming scan requests.
// tx is handed to event handlers; the coroutine receives
// requests and fans out scans.
let mut tree = use_signal(|| DirectoryTree::new(root));

let scan_tx = use_coroutine(|mut rx: UnboundedReceiver<ScanRequest>| {
    async move {
        while let Some(req) = rx.next().await {
            let result: Result<Vec<LoadedEntry>, Error> =
                spawn_blocking(move || {
                    swdir::scan_dir(&req.path)
                        .map(normalize_entries)
                        .map_err(Error::from)
                })
                .await
                .unwrap_or_else(|_| Err(Error::SpawnFailed));

            let payload = LoadPayload {
                path: req.path,
                generation: req.generation,
                result: Arc::new(result),
                depth: req.depth,
            };
            // Merge on the main thread via signal write.
            tree.write().on_loaded(payload);
        }
    }
});
```

When the user expands a folder, the event handler sends a
`ScanRequest` to the coroutine:

```rust
let on_toggle = move |path: PathBuf| {
    let mut t = tree.write();
    // on_toggled returns the generation + path if a scan
    // should be issued; returns None if it's a no-op
    // (already loaded, or collapse).
    if let Some(req) = t.on_toggled(path) {
        scan_tx.send(req);
    }
};
```

### API design recommendation for the port

Expose `on_toggled` and `on_loaded` as methods that:
- Mutate state synchronously.
- Return a `ScanRequest` if a scan needs to be issued, or
  `None` if no I/O is needed.
- Do NOT spawn tasks themselves.

The component wires the requests to the coroutine. This is the
same layering the iced reference implementation uses — state
transitions are synchronous pure-ish functions; side-effects
are produced as data and dispatched separately.

---

## Prefetch in Dioxus

Prefetch produces multiple scan requests. The return type of
`on_loaded` should be:

```rust
struct LoadedResult {
    prefetch_requests: Vec<ScanRequest>,
}
```

The coroutine that processes a completed scan should fan out
the returned prefetch requests:

```rust
// Still inside the coroutine:
let loaded = tree.write().on_loaded(payload);
for req in loaded.prefetch_requests {
    // Each prefetch scan is also handled by this coroutine.
    // Because they're all sent through the same channel,
    // they'll be processed in order; but spawn_blocking
    // calls within each scan can run concurrently.
    scan_tx.send(req);
}
```

---

## Rendering (view layer)

### iced approach

`tree.view(Message::Tree)` returns an `Element<Message>` —
the entire subtree of iced widgets for the current frame.

### Dioxus approach

Dioxus components return RSX, not a generic `Element`. The
simplest port exposes a `DirectoryTreeView` component:

```rust
#[component]
fn DirectoryTreeView(
    tree: Signal<DirectoryTree>,
    on_event: EventHandler<DirectoryTreeEvent>,
) -> Element {
    let rows = tree.read().visible_rows();
    rsx! {
        div {
            class: "directory-tree",
            style: "overflow-y: auto; height: 100%;",
            for (node, depth) in rows {
                TreeRow {
                    key: "{node.path.display()}",
                    node: node.clone(),
                    depth,
                    on_event,
                }
            }
        }
    }
}
```

### Icon rendering

The icon theme is consulted per-row during rendering:

```rust
let spec = tree.read().icon_theme().glyph(role);
rsx! {
    span {
        style: "font-family: {spec.font_name}; font-size: {spec.size}px;",
        "{spec.glyph}"
    }
}
```

Where `spec.font_name` is derived from the `iced::Font` (or
from a Dioxus-specific `FontSpec` if you decouple from iced
types). The Dioxus port does not need to depend on iced's
`Font` type — redefine `IconSpec.font` as `Option<&'static str>`
(a CSS font-family string) for simpler rendering.

---

## Keyboard navigation

### iced approach

iced uses a `subscription()` that listens to the global
keyboard stream. The application calls `tree.handle_key(&key, mods)`
inside its update function.

### Dioxus approach

Dioxus can attach `onkeydown` to a focusable container:

```rust
rsx! {
    div {
        tabindex: "0",  // Make the div focusable
        onkeydown: move |evt| {
            let key = evt.key();
            let mods = evt.modifiers();
            // Translate web KeyboardEvent to a TreeKey action
            if let Some(tree_event) = handle_tree_key(&key, mods) {
                let maybe_event = tree.read().handle_key_action(tree_event);
                if let Some(ev) = maybe_event {
                    on_event.call(ev);
                }
            }
        },
        DirectoryTreeView { tree, on_event }
    }
}
```

Alternatively, listen for global keyboard events via a
`use_future` that bridges with JavaScript's
`addEventListener('keydown', …)`.

---

## Drag-and-drop

### iced approach

iced's `mouse_area` widget exposes `on_press`, `on_release`,
`on_enter`, `on_exit` on a row, enabling the widget to
implement its own drag detection.

### Dioxus / HTML approach

HTML drag-and-drop via `ondragstart` / `ondragover` /
`ondrop` is an option, but it has severe limitations:
- No custom drag images for multi-selection.
- Browser imposes its own cursor handling.
- Doesn't integrate well with the widget's target-validity
  logic.

**Recommended approach: synthesise from mouse events.**

```rust
// On each row:
onmousedown: move |_| {
    on_event.call(DirectoryTreeEvent::Drag(DragMsg::Pressed(path.clone(), is_dir)));
}
onmouseenter: move |_| {
    on_event.call(DirectoryTreeEvent::Drag(DragMsg::Entered(path.clone())));
}
onmouseleave: move |_| {
    on_event.call(DirectoryTreeEvent::Drag(DragMsg::Exited(path.clone())));
}
// On the document root (or a global overlay during drag):
onmouseup: move |_| {
    let target = tree.read().drop_target().map(|p| p.to_path_buf());
    if let Some(dest) = target {
        on_event.call(DirectoryTreeEvent::Drag(DragMsg::Released(dest)));
    } else {
        on_event.call(DirectoryTreeEvent::Drag(DragMsg::Cancelled));
    }
}
```

A global `onmousemove` + `onmouseup` listener can be
mounted/unmounted when a drag begins/ends. Mount it on the
document body or a full-screen overlay div so drop events are
captured even when the mouse leaves the tree.

---

## Dependency considerations

### Does `dioxus-swdir-tree` need to depend on iced?

No. The core state machine has no iced dependency. Extract the
state types into a `swdir-tree-core` crate:

```
swdir-tree-core
├── DirectoryTree (state only, no view)
├── TreeNode, TreeConfig, TreeCache
├── DirectoryTreeEvent, LoadPayload
├── SelectionMode, DragMsg, DragState
├── SearchState
├── IconTheme, IconRole, IconSpec, UnicodeTheme
├── ScanExecutor (trait only)
└── scan_dir wrapper

iced-swdir-tree
└── depends on swdir-tree-core
└── view: tree.view(mapper) -> iced::Element

dioxus-swdir-tree
└── depends on swdir-tree-core
└── component: DirectoryTreeView { tree, on_event }
```

This approach:
- Prevents iced being a transitive dependency of dioxus apps.
- Lets both `iced-swdir-tree` and `dioxus-swdir-tree` share
  the same state machine, tests, and correctness properties.
- Reuses all existing tests in `swdir-tree-core` — a Dioxus
  port doesn't need to re-verify state transitions.

### What must `swdir-tree-core` NOT contain?

- No `iced::Font`, `iced::Element`, `iced::Task`, `iced::Color`.
- `IconSpec.font` should use `Option<CssFontSpec>` (a string)
  or a generic `F: IconFont` bound, not `iced::Font`.
- `ScanFuture` can use `Pin<Box<dyn Future<Output = Result<…>>>>`.

### What `iced` types appear in the current implementation?

| Type | Location | Notes |
| --- | --- | --- |
| `iced::Font` | `IconSpec.font` | Must be replaced in core |
| `iced::Element` | `icon::render` return type | View only; not in core |
| `iced::Task` | `update()` return type | View/binding layer only |
| `iced::widget::*` | `view.rs` | View only |

---

## Testing strategy for the port

The specification in [feature-specs.md](feature-specs.md) is
the oracle. The test patterns in `iced-swdir-tree` are
framework-agnostic and can be reused directly:

```
tests/
├── prefetch.rs       ← can be copied unchanged to swdir-tree-core
├── search.rs         ← can be copied unchanged
├── icon_theme.rs     ← can be copied unchanged
├── tree_multi_select.rs  ← can be copied unchanged
├── tree_drag_drop.rs     ← can be copied unchanged
└── …
```

The `__test_expand_blocking` helper (synchronously scans a
path and merges the result) is the key: it lets tests bypass
the async scanning infrastructure entirely. If `swdir-tree-core`
exposes the same helper, all existing tests port with minimal
changes.

For Dioxus-specific tests (render output, event dispatch),
`dioxus::prelude::VirtualDom` can be used in tests without a
real browser.

---

## Migration checklist

For a Dioxus port developer working through the feature list:

- [ ] **Core state machine** — `DirectoryTree`, `TreeNode`,
  `on_toggled`, `on_loaded`, generation counter, `set_filter`,
  `sync_selection_flags`. No async, no render.
- [ ] **`__test_expand_blocking` helper** — needed for tests.
- [ ] **Selection** — `SelectionMode`, `Selected` handler,
  `visible_rows()`.
- [ ] **Async scanning** — `ScanExecutor`, `ScanJob`,
  coroutine wiring.
- [ ] **Dioxus component** — `DirectoryTreeView`, row rendering,
  caret / icons.
- [ ] **Keyboard** — `onkeydown` handler, `handle_key` logic.
- [ ] **Multi-select** — Shift/Ctrl modifier tracking (must be
  in component state, not tree state).
- [ ] **Drag-and-drop** — synthetic mouse-event drag, target
  validity, `DragCompleted` emission.
- [ ] **Prefetch** — `with_prefetch_limit`, skip list, cascade
  prevention. Tests are already written in `tests/prefetch.rs`.
- [ ] **Incremental search** — `set_search_query`,
  `recompute_search_visibility`, `visible_rows()` dispatch.
  Tests are already written in `tests/search.rs`.
- [ ] **Icon themes** — `IconTheme` trait, `UnicodeTheme`,
  optional `LucideTheme`. Replace `iced::Font` with a
  CSS-compatible alternative.
