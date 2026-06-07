# Core design

## What it is

`iced-swdir-tree` is a stateful **directory-tree widget** for
GUI applications. Its job is to display a navigable tree of
files and directories, accept user input (clicks, keyboard,
drag gestures, type-ahead search), and emit typed events that
the application reacts to.

The widget operates on a single mounted root path. It loads
directory contents lazily — one level at a time, in response
to user expansion — using the `swdir` crate's `scan_dir`
primitive, which lists a single directory non-recursively.

## What it is not

- **Not a filesystem operator.** The widget never creates,
  deletes, renames, moves, or writes anything. Drag-and-drop
  produces a `DragCompleted { sources, destination }` event;
  the *application* does the move.
- **Not a search indexer.** Incremental search filters the
  already-loaded node graph; it never triggers new filesystem
  scans.
- **Not a recursive scanner.** Depth is one level per user
  gesture (or one level per prefetch wave). There is no
  `walk_dir` — only `scan_dir`.
- **Not a file watcher.** The widget has no inotify / FSEvents
  / ReadDirectoryChanges watcher. Refreshing a subtree requires
  an explicit application action (toggle the folder closed and
  open again).

## Core principles

### 1. Non-blocking I/O

The UI thread must never block on disk I/O. Every scan runs on
a worker thread (or task, depending on the executor) and
delivers results back to the UI through an event/message
channel. The `ScanExecutor` trait is the seam that decouples
"which worker runs the scan" from "what the widget does with the
result."

In the reference implementation (iced), this is achieved via
`iced::Task::perform`. What matters is the contract: the
callback that merges results into widget state always runs on
the main thread (or inside the reactive update cycle), never
concurrently with other state mutations.

### 2. Generation tags prevent stale updates

Every scan result carries a `generation: u32` integer.
The widget's own generation counter increments each time a new
scan is issued. When a result arrives, it is accepted if and
only if `payload.generation == tree.generation`. Any mismatch
means the result is stale (issued before a collapse/re-expand
cycle, before a prefetch that was superseded, etc.) and is
silently discarded.

This is the fundamental safety property of the async model:
the widget never corrupts its state with out-of-order results,
even if the application's async runtime delivers them in
arbitrary order.

### 3. Selection is by path, not by node

`TreeNode` objects are ephemeral. They are created when a
directory is expanded and recreated when the display filter
changes or a re-scan merges fresh data. The *selected paths*,
however, are held in a separate, authoritative `Vec<PathBuf>`
on the widget root. Per-node `is_selected` flags are derived
from that Vec on every state mutation that rebuilds nodes.

Consequence: filter changes, re-scans, and collapses preserve
the full selection set. A path may be selected even though its
node is not currently visible or loaded.

### 4. The widget owns UI state; the application owns data state

The widget tracks which folders are open, what is selected,
what is being searched, and whether a drag is in progress.

The application tracks the actual file content, decides what
to do with a `DragCompleted` event (move vs. copy vs. reject),
and decides when to refresh a subtree after an external change.

This is a hard line. The widget will not:
- Perform file operations.
- Watch the filesystem for external changes.
- Re-scan automatically after a drag completes.

Applications that need those behaviours must trigger a re-scan
by toggling the affected folder closed and open.

### 5. Every feature is orthogonal

The widget maintains four largely-independent state dimensions:

| Dimension | State |
| --- | --- |
| Loading | Which folders are expanded/loaded; in-flight generation |
| Selection | The `Vec<PathBuf>` authoritative set; per-node flags |
| Search | Active query; cached `HashSet<PathBuf>` of visible paths |
| Drag | In-progress drag source paths; hovered drop target |

Changing one dimension does not reset any other. A search that
hides a row does not deselect it. A filter flip that rebuilds
nodes preserves the selection and expansion state. A drag does
not clear the search. This composability is deliberate and must
be preserved in any faithful port.

### 6. The root is always in memory

The root `TreeNode` is created eagerly in the constructor and
is never removed or replaced. All mutations are sub-tree
operations. This invariant simplifies the whole implementation:
there is always a valid entry point to walk the tree.

## Responsibility split in detail

| Concern | Widget | Application |
| --- | --- | --- |
| List a directory | ✅ (via executor) | Supplies the executor |
| Display the tree | ✅ | Hosts the widget in its view |
| Track expansion state | ✅ | |
| Track selection | ✅ | |
| Route keyboard to tree | Exposes `handle_key` | Subscribes to keyboard events and calls `handle_key` |
| Move files on drop | | ✅ (react to `DragCompleted`) |
| Refresh after external change | | ✅ (re-toggle the folder) |
| Provide icon font bytes | | Registers with iced/font stack |
| Choose icon theme | Holds default | `with_icon_theme` to override |
| Provide search text input | | ✅ |
| Update search on keystroke | Calls `set_search_query` | Wires TextInput → `set_search_query` |

## The scan lifecycle

```
  user.click(folder)
       │
       ▼
  tree.generation += 1
  issue scan(path, generation) ──► worker thread: scan_dir(path)
       │                                   │
       │◄──────── LoadPayload ─────────────┘
       │            {path, generation, result}
       ▼
  if payload.generation != tree.generation → discard
  else → merge into TreeNode.children
         set is_loaded = true
         sync selection flags
         recompute search visibility (if active)
         trigger prefetch targets (if configured)
```

Each step that produces output (merge → sync → search →
prefetch) is a pure state transition. Side effects (spawning
more scan tasks) happen only at the two marked points: the
initial scan issue and the prefetch issue. Everything else is
in-memory.

## What "loaded" vs. "expanded" means

These two flags on `TreeNode` are frequently confused:

- **`is_loaded`**: the node's children are in memory. Set once
  when the first scan of a path completes. Never cleared unless
  the widget is torn down. Even if the user collapses a folder,
  `is_loaded` remains true — the children stay in the cache.
- **`is_expanded`**: the children are drawn on screen. Set when
  the user clicks the caret; cleared when they click it again.
  A folder can be loaded-but-collapsed (common after prefetch)
  or expanded-but-not-yet-loaded (briefly, while a scan is in
  flight).

Prefetch exploits this: it sets `is_loaded` without touching
`is_expanded`, so the user's click later is a no-I/O fast path.
