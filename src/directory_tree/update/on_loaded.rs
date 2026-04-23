//! Handler for [`DirectoryTreeEvent::Loaded`] — merging a completed
//! async scan result back into the tree.

use crate::directory_tree::DirectoryTree;
use crate::directory_tree::message::LoadPayload;
use crate::directory_tree::node::{LoadedEntry, TreeNode};

impl DirectoryTree {
    /// Merge the result of a completed scan into the tree.
    ///
    /// Stale results (generation mismatch, or node already unloaded
    /// by a collapse) are discarded silently.
    pub(super) fn on_loaded(&mut self, payload: LoadPayload) {
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
