//! Handler for [`DirectoryTreeEvent::Loaded`] — merging a completed
//! async scan result back into the tree.
//!
//! The return value is a v0.5 addition: a list of paths the tree
//! wants prefetch-scanned next. The dispatcher in `update.rs`
//! converts that list into a `Task::batch` of scans. `on_loaded`
//! itself never touches the executor or emits tasks, so it stays a
//! pure state-transition function that's trivial to unit-test.

use std::path::PathBuf;

use crate::directory_tree::DirectoryTree;
use crate::directory_tree::message::LoadPayload;
use crate::directory_tree::node::{LoadedEntry, TreeNode};

impl DirectoryTree {
    /// Merge the result of a completed scan into the tree and return
    /// the paths (if any) that should next be prefetch-scanned.
    ///
    /// Stale results (generation mismatch, or node already unloaded
    /// by a collapse) are discarded silently and return an empty Vec.
    ///
    /// The returned Vec is populated iff v0.5 prefetch is enabled
    /// (`config.prefetch_per_parent > 0`) **and** the payload came
    /// from a user-initiated scan (i.e. `path` is NOT in
    /// `prefetching_paths`). This prevents exponential cascade: a
    /// prefetch-triggered scan result doesn't re-trigger its own
    /// wave of prefetches.
    pub(super) fn on_loaded(&mut self, payload: LoadPayload) -> Vec<PathBuf> {
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
            return Vec::new();
        }

        let Some(node) = self.root.find_mut(&path) else {
            // Node gone (unlikely — the tree persists folders — but
            // defensively bail).
            return Vec::new();
        };
        if !node.is_dir {
            return Vec::new();
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
            self.cache.put(path.clone(), generation, entries.clone());
        }

        // The newly-loaded children may contain (or *be*) one of the
        // selected paths — e.g. the user selected `/a/b/c`, we were
        // showing it via a cached parent, then a re-scan replaced
        // the cache entry with fresh nodes where `is_selected` is
        // false. Re-sync from the authoritative set so view flags
        // stay consistent with what `selected_paths()` returns.
        self.sync_selection_flags();

        // v0.5: prefetch. If THIS load was itself triggered by a
        // prefetch (path is in `prefetching_paths`), we mark it
        // drained and return empty — we do NOT cascade into another
        // wave of prefetches. If THIS load was a user-initiated scan,
        // compute prefetch targets from the freshly-loaded children.
        if self.prefetching_paths.remove(&path) {
            // Prefetch-triggered scan. Children are cached; user's
            // eventual expand of any of them will be instant. No
            // cascade.
            return Vec::new();
        }
        self.select_prefetch_targets(&path)
    }

    /// Identify up to `config.prefetch_per_parent` folder-children
    /// of `parent` whose children aren't yet loaded. These are the
    /// paths the dispatcher will issue background scans for.
    ///
    /// Returns an empty Vec when prefetch is disabled, when the
    /// parent node is missing from the tree, or when every
    /// folder-child is already loaded. Respects `config.max_depth`
    /// by skipping targets that would exceed it.
    pub(super) fn select_prefetch_targets(&self, parent: &std::path::Path) -> Vec<PathBuf> {
        let limit = self.config.prefetch_per_parent;
        if limit == 0 {
            return Vec::new();
        }
        let Some(node) = find_ref(&self.root, parent) else {
            return Vec::new();
        };
        let max_depth = self.config.max_depth;
        let root = &self.config.root_path;
        node.children
            .iter()
            .filter(|c| c.is_dir && !c.is_loaded && c.error.is_none())
            .filter(|c| match max_depth {
                None => true,
                Some(cap) => super::depth_of(root, &c.path) <= cap,
            })
            .take(limit)
            .map(|c| c.path.clone())
            .collect()
    }
}

/// Read-only node lookup (mirror of `TreeNode::find_mut` without the
/// mutable borrow). Used by `select_prefetch_targets`, which needs
/// to read children but doesn't need to mutate.
fn find_ref<'a>(node: &'a TreeNode, target: &std::path::Path) -> Option<&'a TreeNode> {
    if node.path == target {
        return Some(node);
    }
    if !target.starts_with(&node.path) {
        return None;
    }
    node.children.iter().find_map(|c| find_ref(c, target))
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
