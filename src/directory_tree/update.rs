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
use super::message::{DirectoryTreeEvent, LoadPayload};
use super::node::{LoadedEntry, TreeNode};
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
            DirectoryTreeEvent::Selected(path, is_dir) => {
                self.on_selected(path, is_dir);
                Task::none()
            }
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
        walker::scan(path, self.generation, depth)
    }

    /// Apply a selection. Selection is single-select, so we clear all
    /// prior selections before setting the new one — but only if the
    /// new path actually exists as a node. A Selected message for a
    /// path that isn't in the tree (stale, filtered-out, etc.) is a
    /// no-op, because clobbering the user's real selection with
    /// nothing would be an unpleasant surprise.
    fn on_selected(&mut self, path: std::path::PathBuf, _is_dir: bool) {
        // Peek first: if the target isn't present we do nothing.
        // `find_mut` is O(depth) thanks to prefix pruning in
        // `TreeNode::find_mut`, so the extra walk is cheap.
        if self.root.find_mut(&path).is_none() {
            return;
        }
        self.root.clear_selection();
        if let Some(node) = self.root.find_mut(&path) {
            node.is_selected = true;
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
    fn selection_is_single() {
        // We build a tree by hand so we don't hit the filesystem.
        let mut tree = DirectoryTree::new(PathBuf::from("/a"));
        tree.root
            .children
            .push(TreeNode::new_root(PathBuf::from("/a/b")));
        tree.root
            .children
            .push(TreeNode::new_root(PathBuf::from("/a/c")));
        tree.root.is_loaded = true;

        let _ = tree.update(DirectoryTreeEvent::Selected(PathBuf::from("/a/b"), true));
        assert!(
            tree.root
                .find_mut(std::path::Path::new("/a/b"))
                .unwrap()
                .is_selected
        );

        let _ = tree.update(DirectoryTreeEvent::Selected(PathBuf::from("/a/c"), true));
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
}
