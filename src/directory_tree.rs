//! The [`DirectoryTree`] state type — holds the tree's nodes, cache, and
//! configuration, and is the owning handle the parent application keeps
//! across frames.
//!
//! The `update` and `view` methods live in their own submodules
//! ([`update`] and [`view`]) so this file stays focused on construction
//! and configuration.

pub(crate) mod config;
pub(crate) mod error;
pub(crate) mod icon;
pub(crate) mod message;
pub(crate) mod node;
pub(crate) mod update;
pub(crate) mod view;
pub(crate) mod walker;

use std::path::PathBuf;

use self::{
    config::{DirectoryFilter, TreeConfig},
    node::{TreeCache, TreeNode},
};

/// A directory tree widget state.
///
/// Hold one `DirectoryTree` per visible tree in your application state.
/// The widget is cheap to construct: [`DirectoryTree::new`] creates only
/// the root node — child folders are scanned lazily when the user
/// expands them.
///
/// ## Lifecycle
///
/// 1. [`DirectoryTree::new`] — build with a root path.
/// 2. Optionally chain [`DirectoryTree::with_filter`] and/or
///    [`DirectoryTree::with_max_depth`] to configure.
/// 3. Call [`DirectoryTree::view`] from your `view` function.
/// 4. Route emitted [`DirectoryTreeEvent`]s through your app's message
///    system and pass them to [`DirectoryTree::update`], which returns an
///    [`iced::Task`] the parent should `.map(..)` back into its own
///    message type.
///
/// [`DirectoryTreeEvent`]: crate::DirectoryTreeEvent
pub struct DirectoryTree {
    /// The root of the tree. Always present even if traversal fails —
    /// failure just surfaces as [`TreeNode::error`] being set on the root.
    pub(crate) root: TreeNode,
    /// Configuration applied uniformly while traversing.
    pub(crate) config: TreeConfig,
    /// Path → already-loaded children cache to avoid re-scanning on
    /// repeated collapse/expand.
    pub(crate) cache: TreeCache,
    /// Monotonically increasing counter used to invalidate stale async
    /// results when the same folder is expanded, collapsed, expanded
    /// again (or when the tree is dropped / replaced).
    pub(crate) generation: u64,
}

impl DirectoryTree {
    /// Create a new tree rooted at `root`.
    ///
    /// Only the root node is created eagerly; the first level of
    /// children is scanned when the user first expands the root (or,
    /// for convenience, when you call [`DirectoryTree::update`] with a
    /// `Toggled(root)` event yourself).
    ///
    /// Defaults: [`DirectoryFilter::FilesAndFolders`], no depth limit.
    pub fn new(root: PathBuf) -> Self {
        let root_node = TreeNode::new_root(root.clone());
        Self {
            root: root_node,
            config: TreeConfig {
                root_path: root,
                filter: DirectoryFilter::default(),
                max_depth: None,
            },
            cache: TreeCache::default(),
            generation: 0,
        }
    }

    /// Set the display filter.
    ///
    /// This is the builder form used at construction. For runtime
    /// filter changes call [`DirectoryTree::set_filter`] — or use this
    /// method with `std::mem::replace` / `std::mem::take`-style moves
    /// if that fits the shape of your state better. Either route
    /// re-derives visible children from the cache, so the tree
    /// updates instantly without re-scanning the filesystem.
    pub fn with_filter(mut self, filter: DirectoryFilter) -> Self {
        self.set_filter(filter);
        self
    }

    /// Limit how deep the widget will load. `depth == 0` means only the
    /// root's direct children are ever loaded; `depth == 1` allows one
    /// more level of descent; and so on. No limit by default.
    pub fn with_max_depth(mut self, depth: u32) -> Self {
        self.config.max_depth = Some(depth);
        self
    }

    /// Change the display filter at runtime. The tree re-derives its
    /// visible children from the unfiltered cache, so the change is
    /// instant — no re-scan, no blocking the UI.
    ///
    /// Expansion and selection state of previously-visible nodes is
    /// lost when their ancestors' listings are rebuilt, which is a
    /// deliberate v0.1 simplification — see the CHANGELOG roadmap for
    /// the plan to preserve it in v0.2.
    pub fn set_filter(&mut self, filter: DirectoryFilter) {
        if self.config.filter == filter {
            return;
        }
        self.config.filter = filter;
        rebuild_from_cache(&mut self.root, &self.cache, filter);
    }

    /// Return the root path.
    pub fn root_path(&self) -> &std::path::Path {
        &self.config.root_path
    }

    /// Return the current filter.
    pub fn filter(&self) -> DirectoryFilter {
        self.config.filter
    }

    /// Return the current max depth, if any.
    pub fn max_depth(&self) -> Option<u32> {
        self.config.max_depth
    }

    /// Return a reference to the currently selected node's path, if any.
    ///
    /// This walks the tree; selection is stored on the nodes themselves,
    /// not kept as a separate cursor, so changing the filter or
    /// reloading a subtree never leaves a dangling selection.
    pub fn selected_path(&self) -> Option<&std::path::Path> {
        self.root.find_selected().map(|n| n.path.as_path())
    }
}

/// Re-derive the `children` list at every already-loaded directory
/// in the tree from the unfiltered cache, applying `filter`.
///
/// Used by [`DirectoryTree::set_filter`] so a filter change is
/// instant. Unloaded directories are skipped — their filter will be
/// applied on first load, which is already correct without any help
/// from here.
fn rebuild_from_cache(node: &mut TreeNode, cache: &node::TreeCache, filter: DirectoryFilter) {
    if node.is_dir && node.is_loaded {
        if let Some(cached) = cache.get(&node.path) {
            node.children = cached
                .raw
                .iter()
                .filter(|e| e.passes(filter))
                .map(TreeNode::from_entry)
                .collect();
        } else {
            // `is_loaded` without a cache line can happen for the
            // error branch (we mark loaded even on failure). Leave
            // the existing `children` slice — the error state is
            // what matters for those.
        }
    }
    for child in &mut node.children {
        rebuild_from_cache(child, cache, filter);
    }
}
