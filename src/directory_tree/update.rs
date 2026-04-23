//! State machine for [`DirectoryTree::update`].
//!
//! Three inputs flow through this module:
//!
//! 1. [`DirectoryTreeEvent::Toggled`] — a folder expand/collapse from
//!    the user. Expansion may trigger a scan (if not already loaded).
//! 2. [`DirectoryTreeEvent::Selected`] — the user picked an item.
//! 3. [`DirectoryTreeEvent::Loaded`] — an asynchronous scan completed.
//!    Internal message, routed back through the parent's message
//!    plumbing.
//!
//! The returned [`iced::Task`] carries only the `Loaded` follow-up (if
//! any); user-facing events never produce further tasks here.

use iced::Task;

use super::DirectoryTree;
use super::drag::{DragMsg, DragState};
use super::message::{DirectoryTreeEvent, LoadPayload};
use super::node::{LoadedEntry, TreeNode};
use super::selection::SelectionMode;
use super::walker;

impl DirectoryTree {
    /// Feed an event into the widget.
    ///
    /// Returns an `iced::Task` the parent should `.map(..)` back into
    /// its own message type. For `Selected` this is always
    /// [`Task::none()`]; for `Toggled` on an unloaded folder it carries
    /// the pending async scan; for `Loaded` it is again
    /// [`Task::none()`].
    ///
    /// Parent apps typically route every tree-related message here
    /// unconditionally:
    ///
    /// ```ignore
    /// fn update(&mut self, msg: MyMessage) -> Task<MyMessage> {
    ///     match msg {
    ///         MyMessage::Tree(e) => self.tree.update(e).map(MyMessage::Tree),
    ///     }
    /// }
    /// ```
    pub fn update(&mut self, msg: DirectoryTreeEvent) -> Task<DirectoryTreeEvent> {
        match msg {
            DirectoryTreeEvent::Toggled(path) => self.on_toggled(path),
            DirectoryTreeEvent::Selected(path, is_dir, mode) => {
                self.on_selected(path, is_dir, mode);
                Task::none()
            }
            DirectoryTreeEvent::Drag(msg) => self.on_drag(msg),
            // `DragCompleted` is a broadcast event: produced by the
            // widget's own state machine inside `on_drag` and
            // re-routed back through the app's message plumbing so
            // the app can react. When it arrives here again the
            // state machine has already cleared the drag, so this
            // branch is a no-op — routing it back unchanged is the
            // safe default for apps that just `.map(...)` every
            // tree event.
            DirectoryTreeEvent::DragCompleted { .. } => Task::none(),
            DirectoryTreeEvent::Loaded(payload) => {
                self.on_loaded(payload);
                Task::none()
            }
        }
    }

    /// Toggle a directory's expansion state.
    ///
    /// Returns a scan task on first expansion; re-expanding an already
    /// scanned folder is instant (children are kept around), and
    /// collapsing never does I/O.
    fn on_toggled(&mut self, path: std::path::PathBuf) -> Task<DirectoryTreeEvent> {
        // Compute depth against the root *first*: we need it for both
        // the depth-limit check and the generated `LoadPayload`. Doing
        // this before the `find_mut` call also avoids holding a
        // mutable borrow of `self.root` while we look at `self.config`.
        let depth = depth_of(&self.config.root_path, &path);
        let Some(node) = self.root.find_mut(&path) else {
            // The toggle refers to a path that no longer exists in
            // the tree — likely a stale message from a previous root.
            // Drop it silently; it's not worth crashing the app.
            return Task::none();
        };
        if !node.is_dir {
            return Task::none();
        }

        // Collapse: just flip the flag, keep children around so we
        // don't re-scan on the next expand.
        if node.is_expanded {
            node.is_expanded = false;
            return Task::none();
        }

        // Expand.
        node.is_expanded = true;

        // Already loaded → no I/O needed.
        if node.is_loaded {
            return Task::none();
        }

        // Depth-cap check: `None` means unbounded; `Some(d)` limits
        // *descent*, i.e. we refuse to scan at depths greater than d.
        if let Some(limit) = self.config.max_depth
            && depth > limit
        {
            node.is_loaded = true; // pretend we loaded — children stay empty
            return Task::none();
        }

        // Bump the generation and issue the scan. The generation is
        // captured by value into the closure so a future re-expansion
        // can invalidate this in-flight result.
        self.generation = self.generation.wrapping_add(1);
        walker::scan(self.executor.clone(), path, self.generation, depth)
    }

    /// Apply a selection request.
    ///
    /// Selection state is stored on the widget
    /// ([`DirectoryTree::selected_paths`] + [`DirectoryTree::active_path`] +
    /// [`DirectoryTree::anchor_path`]); the per-node `is_selected`
    /// flags used by the view are synced at the end of every mutation.
    ///
    /// Behaviour by mode:
    ///
    /// * [`SelectionMode::Replace`] — clear everything and select
    ///   just `path`. Updates both anchor and active.
    /// * [`SelectionMode::Toggle`] — add `path` to the set if absent,
    ///   remove it otherwise. Updates anchor and active.
    /// * [`SelectionMode::ExtendRange`] — replace the set with every
    ///   visible row between the anchor and `path`. Does **not**
    ///   move the anchor; only active is updated to `path`. Falls
    ///   back to `Replace` semantics if no anchor is set or if
    ///   either endpoint is not currently visible.
    ///
    /// A request for a `path` that isn't present in the tree at all
    /// (stale click, filtered-out node, etc.) is a **no-op** — it
    /// leaves existing selection untouched, to avoid clobbering the
    /// user's real selection with a stale event.
    fn on_selected(&mut self, path: std::path::PathBuf, _is_dir: bool, mode: SelectionMode) {
        // Gate every mode on "target exists in the tree". This is
        // the v0.2 guard that stopped stale clicks from clobbering
        // selection; we keep it for v0.3 across every mode.
        if self.root.find_mut(&path).is_none() {
            return;
        }

        match mode {
            SelectionMode::Replace => {
                self.selected_paths.clear();
                self.selected_paths.push(path.clone());
                self.active_path = Some(path.clone());
                self.anchor_path = Some(path);
            }
            SelectionMode::Toggle => {
                if let Some(pos) = self.selected_paths.iter().position(|p| p == &path) {
                    self.selected_paths.remove(pos);
                } else {
                    self.selected_paths.push(path.clone());
                }
                // Regardless of add-vs-remove, the anchor now tracks
                // the most recently ctrl-clicked item — that's the
                // pivot Windows Explorer and friends use for a
                // subsequent Shift+click.
                self.active_path = Some(path.clone());
                self.anchor_path = Some(path);
            }
            SelectionMode::ExtendRange => {
                let range = self.compute_visible_range(&path);
                match range {
                    Some(paths) => {
                        self.selected_paths = paths;
                        self.active_path = Some(path);
                        // anchor intentionally unchanged
                    }
                    None => {
                        // Fallback: behave as Replace. We don't move
                        // the anchor here either — it wasn't usable,
                        // so the user has not implicitly chosen a
                        // new pivot, and "picking a new pivot
                        // quietly" would be surprising.
                        self.selected_paths.clear();
                        self.selected_paths.push(path.clone());
                        self.active_path = Some(path.clone());
                        self.anchor_path = Some(path);
                    }
                }
            }
        }

        // Refresh per-node view flags.
        self.sync_selection_flags();
    }

    /// Build the list of paths between the anchor and `target`
    /// along the visible-rows order.
    ///
    /// Returns `None` when the operation can't be expressed as a
    /// range — no anchor, or either endpoint not visible. In that
    /// case the caller falls back to `Replace` semantics.
    fn compute_visible_range(&self, target: &std::path::Path) -> Option<Vec<std::path::PathBuf>> {
        let anchor = self.anchor_path.as_deref()?;
        let rows = self.root.visible_rows();
        let a_idx = rows.iter().position(|r| r.node.path == anchor)?;
        let t_idx = rows.iter().position(|r| r.node.path == target)?;
        let (lo, hi) = if a_idx <= t_idx {
            (a_idx, t_idx)
        } else {
            (t_idx, a_idx)
        };
        Some(rows[lo..=hi].iter().map(|r| r.node.path.clone()).collect())
    }

    /// Drive the drag state machine.
    ///
    /// The five [`DragMsg`] variants drive the lifecycle:
    ///
    /// * `Pressed(p, is_dir)` — enter the Dragging state with
    ///   sources derived from the current selection (if `p` is
    ///   already selected) or from `p` alone (if it isn't).
    /// * `Entered(p)` — if `p` is a valid drop target, set it as
    ///   the hover.
    /// * `Exited(p)` — clear hover if it was pointing at `p`.
    /// * `Released(p)` — finalize the gesture:
    ///   - Same row as press? Emit a delayed `Selected(Replace)`
    ///     so the click behaves the way a v0.2/v0.3 single-click
    ///     would.
    ///   - Different row with a valid hover? Emit `DragCompleted`.
    ///   - Anywhere else? Quietly drop back to Idle.
    /// * `Cancelled` — drop to Idle unconditionally.
    ///
    /// `Released` and `Cancelled` are idempotent: they do nothing
    /// if no drag is in progress. The others are also safe to call
    /// out of order — the state machine silently ignores bogus
    /// sequences rather than panicking, so a stray `Entered` with
    /// no prior `Pressed` is a no-op.
    fn on_drag(&mut self, msg: DragMsg) -> Task<DirectoryTreeEvent> {
        match msg {
            DragMsg::Pressed(path, is_dir) => {
                // If the pressed row is already part of the
                // selection, drag the whole selection. Otherwise
                // drag only that row — this matches Explorer /
                // Finder behaviour and avoids accidentally dragging
                // an unrelated set when the user clicks a
                // previously-unselected row.
                let sources: Vec<std::path::PathBuf> = if self.is_selected(&path) {
                    self.selected_paths.clone()
                } else {
                    vec![path.clone()]
                };
                self.drag = Some(DragState {
                    sources,
                    primary: path,
                    primary_is_dir: is_dir,
                    hover: None,
                });
                Task::none()
            }
            DragMsg::Entered(path) => {
                if let Some(d) = self.drag.as_mut() {
                    // Look up whether `path` is a directory. We
                    // have to do this dynamically because the view
                    // doesn't bundle `is_dir` into `Entered`
                    // (keeping the payload small).
                    let is_dir = self.root.find_mut(&path).map(|n| n.is_dir).unwrap_or(false);
                    if d.is_valid_target(&path, is_dir) {
                        d.hover = Some(path);
                    } else {
                        d.hover = None;
                    }
                }
                Task::none()
            }
            DragMsg::Exited(path) => {
                if let Some(d) = self.drag.as_mut()
                    && d.hover.as_deref() == Some(path.as_path())
                {
                    d.hover = None;
                }
                Task::none()
            }
            DragMsg::Released(path) => {
                let Some(d) = self.drag.take() else {
                    return Task::none();
                };
                // Case 1: same-row release. The user pressed and
                // released without ever crossing into another row,
                // i.e., it was a click. Dispatch a delayed
                // `Selected` with Replace mode.
                if path == d.primary {
                    return Task::done(DirectoryTreeEvent::Selected(
                        d.primary,
                        d.primary_is_dir,
                        SelectionMode::Replace,
                    ));
                }
                // Case 2: release over a valid drop target. Emit
                // `DragCompleted` for the app to act on.
                if let Some(dest) = d.hover {
                    return Task::done(DirectoryTreeEvent::DragCompleted {
                        sources: d.sources,
                        destination: dest,
                    });
                }
                // Case 3: release somewhere that wasn't a valid
                // target and wasn't the press row — cancelled drag.
                // Selection is deliberately NOT modified (the user
                // may have been trying to drag the current
                // multi-selection and aborted; preserving state is
                // less surprising than silently collapsing).
                Task::none()
            }
            DragMsg::Cancelled => {
                self.drag = None;
                Task::none()
            }
        }
    }

    /// Merge the result of a completed scan into the tree.
    ///
    /// Stale results (generation mismatch, or node already unloaded
    /// by a collapse) are discarded silently.
    fn on_loaded(&mut self, payload: LoadPayload) {
        let LoadPayload {
            path,
            generation,
            depth: _,
            result,
        } = payload;

        // Stale-generation guard: if the user collapsed and re-expanded
        // between issuing and receiving this scan, a NEWER scan is
        // in flight — keep that one and throw this result away.
        //
        // We compare to `self.generation` with a wrapping_add shift of
        // 0, i.e. strict inequality: a scan result whose generation
        // doesn't match the *current* counter is necessarily stale.
        if generation != self.generation {
            return;
        }

        let Some(node) = self.root.find_mut(&path) else {
            // Node gone (unlikely — the tree persists folders — but
            // defensively bail).
            return;
        };
        if !node.is_dir {
            return;
        }

        // Dropping the Arc back into an owned value for convenience.
        // `result` is `Arc<Result<Vec<LoadedEntry>, Error>>`.
        match result.as_ref() {
            Ok(entries) => {
                node.children = build_children(entries, self.config.filter);
                node.error = None;
            }
            Err(err) => {
                // Permission denied, path gone, etc. — mark the node
                // with the error rather than leaving it in a limbo
                // "expanded but childless" state. The view greys it out.
                node.children.clear();
                node.error = Some(err.clone());
            }
        }
        node.is_loaded = true;

        // Cache raw entries if any. Successful empties are cached too
        // (an explicit Ok(vec![])). The cache holds the **unfiltered**
        // listing so `set_filter` can re-derive children without
        // another scan.
        if let Ok(entries) = result.as_ref() {
            self.cache.put(path, generation, entries.clone());
        }

        // The newly-loaded children may contain (or *be*) one of the
        // selected paths — e.g. the user selected `/a/b/c`, we were
        // showing it via a cached parent, then a re-scan replaced
        // the cache entry with fresh nodes where `is_selected` is
        // false. Re-sync from the authoritative set so view flags
        // stay consistent with what `selected_paths()` returns.
        self.sync_selection_flags();
    }
}

/// Build a child node list from a flat vec of loaded entries, applying
/// the display filter in the process.
fn build_children(entries: &[LoadedEntry], filter: crate::DirectoryFilter) -> Vec<TreeNode> {
    entries
        .iter()
        .filter(|e| e.passes(filter))
        .map(TreeNode::from_entry)
        .collect()
}

/// Compute the depth of `path` relative to `root`. Returns `0` if they
/// are equal, `1` for an immediate child, etc. If `path` does not
/// start with `root` (shouldn't happen in practice — every known node
/// descends from the root) we return `u32::MAX` so any depth limit
/// will trivially exclude it.
fn depth_of(root: &std::path::Path, path: &std::path::Path) -> u32 {
    let Ok(rel) = path.strip_prefix(root) else {
        return u32::MAX;
    };
    rel.components().count() as u32
}

impl DirectoryTree {
    /// Synchronously scan `path` and merge the result.
    ///
    /// **Test/helper API.** This duplicates the async `Toggled → scan →
    /// Loaded` round-trip but blocks on the scan, which is what
    /// integration tests need — iced's `Task` runtime machinery is
    /// private (see `iced_runtime::task::into_stream`), so driving a
    /// Task to completion from outside iced requires either standing
    /// up a window (overkill for unit-level tests) or bypassing the
    /// Task. This method does the latter.
    ///
    /// Real applications should not call this on the main thread —
    /// `scan_dir` blocks on `readdir` — and should route events
    /// through [`DirectoryTree::update`] instead, which delegates the
    /// scan to a worker thread.
    #[doc(hidden)]
    pub fn __test_expand_blocking(&mut self, path: std::path::PathBuf) {
        use super::message::LoadPayload;
        use std::sync::Arc;

        let depth = depth_of(&self.config.root_path, &path);
        // Skip the walker::scan Task entirely: call the blocking
        // primitive directly and hand-assemble the Loaded payload.
        let result = swdir::scan_dir(&path)
            .as_ref()
            .map(|e| super::walker::normalize_entries(e))
            .map_err(crate::Error::from);

        // Make sure generation matches — bump first, then attach.
        self.generation = self.generation.wrapping_add(1);
        let payload = LoadPayload {
            path: path.clone(),
            generation: self.generation,
            depth,
            result: Arc::new(result),
        };

        // Flip is_expanded, then feed the payload through the real
        // on_loaded so caching, error handling, etc. all go through
        // the production code path.
        if let Some(node) = self.root.find_mut(&path)
            && node.is_dir
        {
            node.is_expanded = true;
        }
        self.on_loaded(payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DirectoryTree;
    use std::path::PathBuf;

    #[test]
    fn toggled_on_nonexistent_path_is_noop() {
        let mut tree = DirectoryTree::new(PathBuf::from("/definitely/not/there"));
        let _task = tree.update(DirectoryTreeEvent::Toggled(PathBuf::from(
            "/some/unrelated/elsewhere",
        )));
        // Just asserting we didn't panic.
    }

    #[test]
    fn selection_is_single_under_replace_mode() {
        // Plain Replace-mode clicks keep single-select semantics:
        // each new selection wipes the previous one.
        let mut tree = DirectoryTree::new(PathBuf::from("/a"));
        tree.root
            .children
            .push(TreeNode::new_root(PathBuf::from("/a/b")));
        tree.root
            .children
            .push(TreeNode::new_root(PathBuf::from("/a/c")));
        tree.root.is_loaded = true;

        let _ = tree.update(DirectoryTreeEvent::Selected(
            PathBuf::from("/a/b"),
            true,
            SelectionMode::Replace,
        ));
        assert!(
            tree.root
                .find_mut(std::path::Path::new("/a/b"))
                .unwrap()
                .is_selected
        );
        assert_eq!(tree.selected_paths.len(), 1);

        let _ = tree.update(DirectoryTreeEvent::Selected(
            PathBuf::from("/a/c"),
            true,
            SelectionMode::Replace,
        ));
        assert!(
            !tree
                .root
                .find_mut(std::path::Path::new("/a/b"))
                .unwrap()
                .is_selected
        );
        assert!(
            tree.root
                .find_mut(std::path::Path::new("/a/c"))
                .unwrap()
                .is_selected
        );
        assert_eq!(tree.selected_paths.len(), 1);
    }

    #[test]
    fn collapsing_keeps_children() {
        let mut tree = DirectoryTree::new(PathBuf::from("/r"));
        tree.root.is_loaded = true;
        tree.root.is_expanded = true;
        tree.root
            .children
            .push(TreeNode::new_root(PathBuf::from("/r/x")));
        let _ = tree.update(DirectoryTreeEvent::Toggled(PathBuf::from("/r")));
        assert!(!tree.root.is_expanded);
        assert_eq!(
            tree.root.children.len(),
            1,
            "children must survive collapse"
        );
    }

    #[test]
    fn stale_loaded_events_are_dropped() {
        let mut tree = DirectoryTree::new(PathBuf::from("/r"));
        tree.root.is_dir = true;
        tree.root.is_expanded = true;
        tree.generation = 5;

        let stale = LoadPayload {
            path: PathBuf::from("/r"),
            generation: 4, // older than current
            depth: 0,
            result: std::sync::Arc::new(Ok(vec![LoadedEntry {
                path: PathBuf::from("/r/hacked"),
                is_dir: false,
                is_symlink: false,
                is_hidden: false,
            }])),
        };
        let _ = tree.update(DirectoryTreeEvent::Loaded(stale));
        assert!(
            tree.root.children.is_empty(),
            "stale result must be ignored"
        );
    }

    // -----------------------------------------------------------------
    // Multi-select (v0.3) state-machine tests.
    //
    // These exercise the three modes directly against the state
    // struct — no filesystem or iced runtime involved.
    // -----------------------------------------------------------------

    /// Build a small hand-made tree for selection tests: /r with
    /// three expanded loaded siblings.
    fn tree_with_three_siblings() -> DirectoryTree {
        let mut tree = DirectoryTree::new(PathBuf::from("/r"));
        tree.root.is_dir = true;
        tree.root.is_expanded = true;
        tree.root.is_loaded = true;
        for name in &["a", "b", "c"] {
            tree.root
                .children
                .push(TreeNode::new_root(PathBuf::from(format!("/r/{}", name))));
        }
        tree
    }

    fn sel(p: &str, mode: SelectionMode) -> DirectoryTreeEvent {
        DirectoryTreeEvent::Selected(PathBuf::from(p), false, mode)
    }

    #[test]
    fn replace_clears_previous_selection() {
        let mut tree = tree_with_three_siblings();
        let _ = tree.update(sel("/r/a", SelectionMode::Replace));
        let _ = tree.update(sel("/r/b", SelectionMode::Replace));
        assert_eq!(tree.selected_paths.len(), 1);
        assert_eq!(tree.selected_paths[0], PathBuf::from("/r/b"));
        assert_eq!(
            tree.active_path.as_deref(),
            Some(std::path::Path::new("/r/b"))
        );
        assert_eq!(
            tree.anchor_path.as_deref(),
            Some(std::path::Path::new("/r/b"))
        );
    }

    #[test]
    fn toggle_adds_then_removes() {
        let mut tree = tree_with_three_siblings();
        let _ = tree.update(sel("/r/a", SelectionMode::Replace));
        let _ = tree.update(sel("/r/b", SelectionMode::Toggle));
        let _ = tree.update(sel("/r/c", SelectionMode::Toggle));
        assert_eq!(
            tree.selected_paths.len(),
            3,
            "three paths after two toggles"
        );

        // Toggle /r/b again — should remove it.
        let _ = tree.update(sel("/r/b", SelectionMode::Toggle));
        assert_eq!(tree.selected_paths.len(), 2);
        assert!(
            !tree
                .selected_paths
                .iter()
                .any(|p| p == std::path::Path::new("/r/b"))
        );

        // Anchor always follows the most recent toggled path
        // regardless of add/remove.
        assert_eq!(
            tree.anchor_path.as_deref(),
            Some(std::path::Path::new("/r/b"))
        );
    }

    #[test]
    fn toggle_updates_per_node_flags() {
        let mut tree = tree_with_three_siblings();
        let _ = tree.update(sel("/r/a", SelectionMode::Replace));
        let _ = tree.update(sel("/r/b", SelectionMode::Toggle));
        // Both /r/a and /r/b should have is_selected = true now.
        assert!(
            tree.root
                .find_mut(std::path::Path::new("/r/a"))
                .unwrap()
                .is_selected
        );
        assert!(
            tree.root
                .find_mut(std::path::Path::new("/r/b"))
                .unwrap()
                .is_selected
        );
        // /r/c untouched.
        assert!(
            !tree
                .root
                .find_mut(std::path::Path::new("/r/c"))
                .unwrap()
                .is_selected
        );
    }

    #[test]
    fn extend_range_covers_visible_interval() {
        let mut tree = tree_with_three_siblings();
        // Anchor at /r/a via a plain Replace.
        let _ = tree.update(sel("/r/a", SelectionMode::Replace));
        // Shift-range to /r/c — should pick up /r, /r/a, /r/b, /r/c
        // (in visible-row order).
        let _ = tree.update(sel("/r/c", SelectionMode::ExtendRange));
        // /r is visible as row 0 but the anchor was /r/a, so range
        // runs from /r/a..=/r/c (3 rows).
        assert_eq!(tree.selected_paths.len(), 3);
        let names: Vec<_> = tree
            .selected_paths
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        assert_eq!(names, vec!["a", "b", "c"]);
        // Anchor must not have moved.
        assert_eq!(
            tree.anchor_path.as_deref(),
            Some(std::path::Path::new("/r/a"))
        );
    }

    #[test]
    fn extend_range_is_symmetric() {
        let mut tree = tree_with_three_siblings();
        // Anchor at /r/c, then extend *backwards* to /r/a.
        let _ = tree.update(sel("/r/c", SelectionMode::Replace));
        let _ = tree.update(sel("/r/a", SelectionMode::ExtendRange));
        assert_eq!(tree.selected_paths.len(), 3);
        // Anchor still on /r/c.
        assert_eq!(
            tree.anchor_path.as_deref(),
            Some(std::path::Path::new("/r/c"))
        );
    }

    #[test]
    fn extend_range_without_anchor_falls_back_to_replace() {
        // Fresh tree, no prior selection/anchor: ExtendRange acts
        // as Replace onto the target.
        let mut tree = tree_with_three_siblings();
        let _ = tree.update(sel("/r/b", SelectionMode::ExtendRange));
        assert_eq!(tree.selected_paths.len(), 1);
        assert_eq!(tree.selected_paths[0], PathBuf::from("/r/b"));
        // Fallback *does* pick a new anchor, so the user can then
        // Shift+click somewhere else and get a meaningful range.
        assert_eq!(
            tree.anchor_path.as_deref(),
            Some(std::path::Path::new("/r/b"))
        );
    }

    #[test]
    fn selection_on_stale_path_is_noop() {
        let mut tree = tree_with_three_siblings();
        let _ = tree.update(sel("/r/a", SelectionMode::Replace));
        // Click on a path that is NOT in the tree — leaves state alone.
        let _ = tree.update(sel("/completely/unrelated", SelectionMode::Replace));
        assert_eq!(tree.selected_paths.len(), 1);
        assert_eq!(tree.selected_paths[0], PathBuf::from("/r/a"));
    }

    // -----------------------------------------------------------------
    // Drag-and-drop (v0.4) state-machine tests.
    // -----------------------------------------------------------------

    /// Build a tree with two folders /r/x, /r/y and a file /r/f.
    /// Used to test "drop onto folder" vs "drop onto file".
    fn tree_with_two_folders_and_a_file() -> DirectoryTree {
        let mut tree = DirectoryTree::new(PathBuf::from("/r"));
        tree.root.is_dir = true;
        tree.root.is_expanded = true;
        tree.root.is_loaded = true;
        let mut x = TreeNode::new_root(PathBuf::from("/r/x"));
        x.is_dir = true;
        tree.root.children.push(x);
        let mut y = TreeNode::new_root(PathBuf::from("/r/y"));
        y.is_dir = true;
        tree.root.children.push(y);
        let mut f = TreeNode::new_root(PathBuf::from("/r/f"));
        // `new_root` defaults is_dir=true (roots are always dirs);
        // override for the file node.
        f.is_dir = false;
        tree.root.children.push(f);
        tree
    }

    #[test]
    fn press_without_prior_selection_drags_only_pressed_row() {
        let mut tree = tree_with_two_folders_and_a_file();
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
            PathBuf::from("/r/f"),
            false,
        )));
        assert!(tree.is_dragging());
        assert_eq!(tree.drag_sources(), &[PathBuf::from("/r/f")]);
    }

    #[test]
    fn press_on_selected_row_drags_whole_selection() {
        // Multi-select {/r/f, /r/x}, then press on /r/f — drag the
        // whole set.
        let mut tree = tree_with_two_folders_and_a_file();
        let _ = tree.update(DirectoryTreeEvent::Selected(
            PathBuf::from("/r/f"),
            false,
            SelectionMode::Replace,
        ));
        let _ = tree.update(DirectoryTreeEvent::Selected(
            PathBuf::from("/r/x"),
            true,
            SelectionMode::Toggle,
        ));
        assert_eq!(tree.selected_paths().len(), 2);

        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
            PathBuf::from("/r/f"),
            false,
        )));
        assert_eq!(
            tree.drag_sources().len(),
            2,
            "pressing a selected row drags the whole selection"
        );
    }

    #[test]
    fn entered_folder_sets_hover_to_folder() {
        let mut tree = tree_with_two_folders_and_a_file();
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
            PathBuf::from("/r/f"),
            false,
        )));
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(PathBuf::from(
            "/r/x",
        ))));
        assert_eq!(tree.drop_target(), Some(std::path::Path::new("/r/x")));
    }

    #[test]
    fn entered_file_leaves_hover_unset() {
        let mut tree = tree_with_two_folders_and_a_file();
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
            PathBuf::from("/r/x"),
            true,
        )));
        // Now cursor moves over a file, not a folder. Not a valid target.
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(PathBuf::from(
            "/r/f",
        ))));
        assert_eq!(tree.drop_target(), None);
    }

    #[test]
    fn entered_source_row_leaves_hover_unset() {
        // Dragging /r/x and cursor re-enters /r/x — can't drop on
        // self. hover stays None.
        let mut tree = tree_with_two_folders_and_a_file();
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
            PathBuf::from("/r/x"),
            true,
        )));
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(PathBuf::from(
            "/r/x",
        ))));
        assert_eq!(tree.drop_target(), None);
    }

    #[test]
    fn exited_target_clears_hover() {
        let mut tree = tree_with_two_folders_and_a_file();
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
            PathBuf::from("/r/f"),
            false,
        )));
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(PathBuf::from(
            "/r/x",
        ))));
        assert_eq!(tree.drop_target(), Some(std::path::Path::new("/r/x")));
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Exited(PathBuf::from(
            "/r/x",
        ))));
        assert_eq!(tree.drop_target(), None);
    }

    #[test]
    fn release_same_row_produces_delayed_selected() {
        // Press on /r/f, release on /r/f → click, not drag.
        // The widget emits a Task<Selected> for the app to
        // observe; the drag state is cleared.
        let mut tree = tree_with_two_folders_and_a_file();
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
            PathBuf::from("/r/f"),
            false,
        )));
        let _task = tree.update(DirectoryTreeEvent::Drag(DragMsg::Released(PathBuf::from(
            "/r/f",
        ))));
        // We can't easily inspect the Task's payload without
        // running an iced runtime, but we *can* confirm the drag
        // state is cleared, which is the state-machine
        // invariant. (The Task<Selected> side effect is exercised
        // end-to-end in the integration tests in tests/tree.rs.)
        assert!(!tree.is_dragging());
    }

    #[test]
    fn release_over_valid_target_clears_drag_state() {
        let mut tree = tree_with_two_folders_and_a_file();
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
            PathBuf::from("/r/f"),
            false,
        )));
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(PathBuf::from(
            "/r/x",
        ))));
        let _task = tree.update(DirectoryTreeEvent::Drag(DragMsg::Released(PathBuf::from(
            "/r/x",
        ))));
        assert!(!tree.is_dragging());
        // The returned Task carries a DragCompleted; we test that
        // end-to-end in the integration tests.
    }

    #[test]
    fn release_without_hover_is_silent_cancel() {
        // Press on /r/f, release on /r/y WITHOUT having entered any
        // row (Entered was never called). hover is None, so it's
        // treated as cancel — state clears, no event.
        let mut tree = tree_with_two_folders_and_a_file();
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
            PathBuf::from("/r/f"),
            false,
        )));
        let _task = tree.update(DirectoryTreeEvent::Drag(DragMsg::Released(PathBuf::from(
            "/r/y",
        ))));
        assert!(!tree.is_dragging());
        // selection must still be empty (no delayed Selected).
        assert!(tree.selected_paths().is_empty());
    }

    #[test]
    fn explicit_cancelled_clears_drag() {
        let mut tree = tree_with_two_folders_and_a_file();
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
            PathBuf::from("/r/f"),
            false,
        )));
        assert!(tree.is_dragging());
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Cancelled));
        assert!(!tree.is_dragging());
    }

    #[test]
    fn stray_events_without_press_are_noops() {
        let mut tree = tree_with_two_folders_and_a_file();
        // No drag active. These must not panic or create state.
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(PathBuf::from(
            "/r/x",
        ))));
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Exited(PathBuf::from(
            "/r/x",
        ))));
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Released(PathBuf::from(
            "/r/x",
        ))));
        let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Cancelled));
        assert!(!tree.is_dragging());
    }
}
