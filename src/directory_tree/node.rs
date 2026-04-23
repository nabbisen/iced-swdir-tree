//! In-memory tree types: [`TreeNode`] and [`TreeCache`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::Error;

/// A single node in the directory tree.
///
/// All fields are public so downstream code (tests, custom renderers
/// built on top of the widget's state) can inspect them, but the widget
/// itself drives mutation through [`DirectoryTree::update`].
///
/// [`DirectoryTree::update`]: crate::DirectoryTree::update
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Full path of the entry.
    pub path: PathBuf,
    /// `true` if the entry is a directory.
    pub is_dir: bool,
    /// `true` if the directory is currently expanded in the UI. Always
    /// `false` for files.
    pub is_expanded: bool,
    /// `true` if this directory has already been scanned at least once,
    /// even if the scan returned zero children. Distinguishes
    /// "scanned and empty" from "not scanned yet" in the view.
    pub is_loaded: bool,
    /// Direct children. Empty until `is_loaded` is `true`.
    pub children: Vec<TreeNode>,
    /// `true` if the user has this node selected.
    pub is_selected: bool,
    /// Populated when a scan of *this directory* failed. For files this
    /// is always `None`. The view layer uses this to render a greyed-out
    /// or error-tinted row.
    pub error: Option<Error>,
}

impl TreeNode {
    /// Build the root node of a freshly-constructed [`DirectoryTree`].
    ///
    /// The root is always treated as a directory (we can't meaningfully
    /// root a tree at a regular file).
    ///
    /// [`DirectoryTree`]: crate::DirectoryTree
    pub(crate) fn new_root(path: PathBuf) -> Self {
        Self {
            path,
            is_dir: true,
            is_expanded: false,
            is_loaded: false,
            children: Vec::new(),
            is_selected: false,
            error: None,
        }
    }

    /// Build a child node from a loaded entry.
    pub(crate) fn from_entry(entry: &LoadedEntry) -> Self {
        Self {
            path: entry.path.clone(),
            is_dir: entry.is_dir,
            is_expanded: false,
            is_loaded: false,
            children: Vec::new(),
            is_selected: false,
            error: None,
        }
    }

    /// Find a descendant (including `self`) by path, returning a
    /// mutable reference.
    ///
    /// Uses path-prefix pruning: we only descend into subtrees that
    /// could contain `target`, so the worst case is O(depth) not
    /// O(total nodes).
    pub(crate) fn find_mut(&mut self, target: &Path) -> Option<&mut TreeNode> {
        if self.path == target {
            return Some(self);
        }
        // Only descend if target lives under `self.path`. Without this
        // check we'd walk every sibling subtree on every lookup.
        if !target.starts_with(&self.path) {
            return None;
        }
        for child in &mut self.children {
            if let Some(hit) = child.find_mut(target) {
                return Some(hit);
            }
        }
        None
    }

    /// Clear the selection flag on every node in this subtree.
    ///
    /// Selection is single-select; setting a new selection is a
    /// clear-then-set operation.
    pub(crate) fn clear_selection(&mut self) {
        self.is_selected = false;
        for child in &mut self.children {
            child.clear_selection();
        }
    }

    /// Count nodes in this subtree (including `self`). Exposed primarily
    /// for tests and diagnostics.
    #[allow(dead_code)]
    pub(crate) fn node_count(&self) -> usize {
        1 + self.children.iter().map(Self::node_count).sum::<usize>()
    }
}

/// Lightweight, owned entry record produced by [`crate::walker`] and
/// consumed by [`super::update`] to build [`TreeNode`]s.
///
/// We don't use `swdir::DirEntry` here directly — keeping swdir types
/// out of the message enum means swdir can be a private dependency
/// from the public API's point of view.
#[derive(Debug, Clone)]
pub struct LoadedEntry {
    /// Full path of the entry.
    pub path: PathBuf,
    /// `true` if the entry is a directory. Symlinks to directories
    /// are treated as files here (the widget never auto-follows them,
    /// to stay robust against cycles).
    pub is_dir: bool,
    /// `true` if the entry itself is a symlink (regardless of target).
    ///
    /// Currently only used for cycle-avoidance diagnostics; kept here
    /// so v0.4+ can render a symlink indicator without having to
    /// re-stat every entry.
    #[allow(dead_code)]
    pub is_symlink: bool,
    /// `true` if the entry is hidden per OS conventions.
    ///
    /// Persisted on the entry (not just consulted in the scan path)
    /// so that a later filter change — e.g. flipping from
    /// `FilesAndFolders` to `AllIncludingHidden` — can be applied
    /// from the cache without another disk scan.
    pub is_hidden: bool,
}

impl LoadedEntry {
    /// `true` if this entry should be visible under `filter`.
    ///
    /// The rules mirror [`DirectoryFilter`](crate::DirectoryFilter)'s
    /// predicates but operate on the per-entry flags we already have
    /// in hand, keeping the decision O(1) rather than touching the
    /// filesystem again.
    pub(crate) fn passes(&self, filter: crate::DirectoryFilter) -> bool {
        if filter.skips_hidden() && self.is_hidden {
            return false;
        }
        if filter.skips_files() && !self.is_dir {
            return false;
        }
        true
    }
}

/// A path → children cache so that collapsing and re-expanding a folder
/// does not re-scan the filesystem.
///
/// The cache stores **unfiltered** children (raw normalised entries).
/// When the filter changes at runtime, [`DirectoryTree::set_filter`]
/// re-derives each already-loaded directory's visible child list from
/// its cached raw entries — no filesystem I/O, no flicker.
///
/// [`DirectoryTree::set_filter`]: crate::DirectoryTree::set_filter
/// (In practice the raw entry set is small — a single directory's
/// listing — so the extra memory cost of keeping both raw and
/// filtered forms is not justified at this scale.)
#[derive(Debug, Default, Clone)]
pub struct TreeCache {
    entries: HashMap<PathBuf, CacheEntry>,
}

/// A single cache line: the raw (unfiltered but normalized) listing
/// of a directory plus the generation number at which it was
/// recorded. Read by
/// [`DirectoryTree::set_filter`](crate::DirectoryTree::set_filter)
/// to re-derive filtered children without another scan.
#[derive(Debug, Clone)]
pub(crate) struct CacheEntry {
    /// Generation this cache line was recorded with. Stale lines are
    /// skipped rather than deleted, to avoid churn on repeat expansions.
    #[allow(dead_code)]
    pub generation: u64,
    /// The raw, unfiltered children of the directory.
    pub raw: Vec<LoadedEntry>,
}

impl TreeCache {
    /// Insert or replace the cached entries for `dir`.
    pub(crate) fn put(&mut self, dir: PathBuf, generation: u64, raw: Vec<LoadedEntry>) {
        self.entries.insert(dir, CacheEntry { generation, raw });
    }

    /// Retrieve the raw entries previously recorded for `dir`, if any.
    pub(crate) fn get(&self, dir: &Path) -> Option<&CacheEntry> {
        self.entries.get(dir)
    }

    /// Drop every cached entry. Used when the filter changes in a way
    /// that could affect membership (hidden → not-hidden, etc.).
    #[allow(dead_code)]
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_mut_self() {
        let mut root = TreeNode::new_root(PathBuf::from("/a"));
        assert!(root.find_mut(Path::new("/a")).is_some());
    }

    #[test]
    fn find_mut_child() {
        let mut root = TreeNode::new_root(PathBuf::from("/a"));
        root.children.push(TreeNode::from_entry(&LoadedEntry {
            path: PathBuf::from("/a/b"),
            is_dir: true,
            is_symlink: false,
            is_hidden: false,
        }));
        root.children[0].is_loaded = true;
        root.children[0]
            .children
            .push(TreeNode::from_entry(&LoadedEntry {
                path: PathBuf::from("/a/b/c"),
                is_dir: false,
                is_symlink: false,
                is_hidden: false,
            }));
        assert!(root.find_mut(Path::new("/a/b/c")).is_some());
    }

    #[test]
    fn find_mut_prunes_unrelated() {
        let mut root = TreeNode::new_root(PathBuf::from("/a"));
        root.children.push(TreeNode::from_entry(&LoadedEntry {
            path: PathBuf::from("/a/b"),
            is_dir: true,
            is_symlink: false,
            is_hidden: false,
        }));
        // target not under /a — should return None without panicking
        assert!(root.find_mut(Path::new("/x/y")).is_none());
    }

    #[test]
    fn clear_selection_recurses() {
        let mut root = TreeNode::new_root(PathBuf::from("/a"));
        root.is_selected = true;
        let mut child = TreeNode::from_entry(&LoadedEntry {
            path: PathBuf::from("/a/b"),
            is_dir: true,
            is_symlink: false,
            is_hidden: false,
        });
        child.is_selected = true;
        root.children.push(child);
        root.clear_selection();
        assert!(!root.is_selected);
        assert!(!root.children[0].is_selected);
    }
}
