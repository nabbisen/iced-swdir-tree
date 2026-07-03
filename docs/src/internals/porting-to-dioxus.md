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

## Cross-framework architecture decision (v0.9.0)

> **Note:** This document previously recommended extracting a shared
> `swdir-tree-core` crate. That recommendation was withdrawn (RFC 003,
> archived). See `HANDOFF.md` for the full rationale.

The resolved position is: **share the design, not the code.**
Each framework implements the shared spec idiomatically, using the
data structures and async model that fit its architecture.

The shared assets are the documents in `docs/design/` — especially
`feature-specs.md` (the S-clause oracle each side's tests validate
against) and `state-machine.md` (the precise transition spec). The
Dioxus core (`dioxus-swdir-tree-core`) was built "following the
design documents of `iced-swdir-tree` v0.7" and demonstrated that
faithful parity is achievable from the docs alone, without a shared
dependency.

`dioxus-swdir-tree-core`'s internal core/view split is the right
call for the Dioxus architecture. It is not a template `iced-swdir-tree`
must copy; each project stays self-contained and follows the shared spec.

---

## Testing strategy for the port

The specification in [feature-specs.md](feature-specs.md) is
the oracle. The test patterns in `iced-swdir-tree` are
behavioural, not iced-specific, and can be ported directly:

```
tests/
├── prefetch.rs            ← port to dioxus-swdir-tree-core tests
├── search.rs              ← port to dioxus-swdir-tree-core tests
├── icon_theme.rs          ← port to dioxus-swdir-tree-core tests
├── tree_multi_select.rs   ← port to dioxus-swdir-tree-core tests
├── tree_drag_drop.rs      ← port to dioxus-swdir-tree-core tests
├── item_tree_drag_drop.rs ← NEW (v0.9.0); port for ItemTree DnD
└── …
```

The `__test_expand_blocking` / `scan_and_feed` helper is the key
for `DirectoryTree` tests. For `ItemTree` tests no such helper is
needed — `ItemTree` is synchronous and its tests drive the state
machine directly.

For Dioxus-specific tests (render output, event dispatch),
`dioxus::prelude::VirtualDom` can be used in tests without a
real browser.

---

## Migration checklist

For a Dioxus port developer working through the feature list.
Items marked ✅ are already complete in `dioxus-swdir-tree` v0.8.0.

**DirectoryTree**

- ✅ Core state machine — `DirectoryTree`, `TreeNode`, `on_toggled`,
  `on_loaded`, generation counter, `set_filter`,
  `sync_selection_flags`.
- ✅ `expand_blocking` helper — for tests.
- ✅ Selection — `SelectionMode`, `on_selected`, `visible_rows()`.
- ✅ Async scanning — `ScanExecutor`, coroutine/`ScanRequest` wiring.
- ✅ Dioxus component — `DirectoryTreeView`, row rendering, caret/icons.
- ✅ Keyboard — `onkeydown` handler, `handle_key` logic.
- ✅ Multi-select — Shift/Ctrl modifier tracking.
- ✅ Drag-and-drop — synthetic mouse-event drag, target validity,
  `DragCompleted` emission.
- ✅ Prefetch — `with_prefetch_limit`, skip list, cascade prevention.
- ✅ Incremental search — `set_search_query`, `visible_rows()`.
- ✅ Icon themes — `IconTheme`, `UnicodeTheme`, optional `LucideTheme`.

**ItemTree**

- ✅ Core state machine — `ItemTree<T>`, `NodeId`, `ItemNode`,
  `set_tree` with key-based diffing, `on_toggled`, `on_selected`.
- ✅ Selection and keyboard — same contract as `DirectoryTree`.
- ✅ Incremental search — matches against display string, `T: Display`.
- ✅ Dioxus component — `ItemTreeView`, `item_view.rs`, `item_row.rs`.
- [ ] **ItemTree drag-and-drop** ← **not yet implemented; primary
  handoff item from iced-swdir-tree v0.9.0.** See `HANDOFF.md` for
  the full spec and `docs/design/feature-specs.md` S11.9–S11.16 for
  the oracle. Summary:
  - `DropPosition { Before, Into, After }` — before/into/after a target.
  - `with_drag_and_drop(bool)` opt-in (default off).
  - Validity: effective-new-parent must not be a source or descendant
    of a source. Use the arena's existing parent links — no snapshot
    needed (simpler than iced's workaround).
  - Events: `ItemTreeEvent::Drag(ItemDragMsg)` +
    `DragCompleted { sources, target, position }`.
  - Deferred selection: press does not mutate; same-node release
    emits `Selected(Replace)`.
  - `Escape` cancels (only while drag active).
  - View: three zones per row — thin `Before` strip, row body (`Into`),
    thin `After` strip — each a separate `onmouseenter/leave/up` target.

