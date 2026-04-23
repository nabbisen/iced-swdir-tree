//! The [`DirectoryTree`] state type — holds the tree's nodes, cache, and
//! configuration, and is the owning handle the parent application keeps
//! across frames.
//!
//! The `update` and `view` methods live in their own submodules
//! ([`update`] and [`view`]) so this file stays focused on construction
//! and configuration.

pub(crate) mod config;
pub(crate) mod error;
pub(crate) mod executor;
pub(crate) mod icon;
pub(crate) mod keyboard;
pub(crate) mod message;
pub(crate) mod node;
pub(crate) mod update;
pub(crate) mod view;
pub(crate) mod walker;

use std::path::PathBuf;
use std::sync::Arc;

use self::{
    config::{DirectoryFilter, TreeConfig},
    executor::{ScanExecutor, ThreadExecutor},
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
    /// Canonical selection cursor.
    ///
    /// This is the single source of truth for which node is selected.
    /// [`TreeNode::is_selected`] is treated as a view-layer cache —
    /// updated in lockstep with this field by the update layer — so the
    /// view can continue to read selection state node-local without
    /// chasing a pointer on every row.
    ///
    /// Keeping selection keyed by path rather than stored on the node
    /// has two payoffs:
    ///
    /// 1. Runtime filter changes survive. `set_filter` rebuilds every
    ///    loaded directory's `children` Vec from the raw cache, which
    ///    constructs brand-new `TreeNode`s; storing selection per-path
    ///    lets us re-mark the correct new node afterwards.
    /// 2. A selected path that becomes invisible (filtered out, or its
    ///    parent collapsed) is not lost — it comes back the moment the
    ///    filter or expansion state makes it visible again.
    pub(crate) selected_path: Option<std::path::PathBuf>,
    /// Pluggable executor that runs blocking `scan_dir` calls.
    ///
    /// Defaults to [`ThreadExecutor`] (one `std::thread::spawn` per
    /// expansion), which is correct but slightly wasteful for apps
    /// that already run their own blocking-task pool. Swap it via
    /// [`DirectoryTree::with_executor`].
    pub(crate) executor: Arc<dyn ScanExecutor>,
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
            selected_path: None,
            executor: Arc::new(ThreadExecutor),
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

    /// Route blocking `scan_dir` calls through a custom executor.
    ///
    /// By default the widget spawns a fresh `std::thread` per
    /// expansion via [`ThreadExecutor`]. Apps that already manage
    /// a blocking-task pool (tokio, smol, rayon, ...) can implement
    /// [`ScanExecutor`] and swap it in here:
    ///
    /// ```ignore
    /// use std::sync::Arc;
    /// let tree = DirectoryTree::new(root).with_executor(Arc::new(MyTokioExecutor));
    /// ```
    ///
    /// Calling this mid-session is allowed (the next scan will use
    /// the new executor); in-flight scans initiated under the old
    /// executor still complete through it.
    ///
    /// [`ScanExecutor`]: crate::ScanExecutor
    /// [`ThreadExecutor`]: crate::ThreadExecutor
    pub fn with_executor(mut self, executor: Arc<dyn ScanExecutor>) -> Self {
        self.executor = executor;
        self
    }

    /// Change the display filter at runtime. The tree re-derives its
    /// visible children from the unfiltered cache, so the change is
    /// instant — no re-scan, no blocking the UI.
    ///
    /// **Selection is preserved.** Selection is kept by path in
    /// [`DirectoryTree::selected_path`], not on the [`TreeNode`]s that
    /// this call rebuilds, so the selection cursor survives the filter
    /// swap. If the selected node becomes invisible under the new
    /// filter it is not lost — flipping the filter back will re-reveal
    /// the selection unchanged.
    ///
    /// **Expansion state is preserved too.** `rebuild_from_cache`
    /// copies `is_expanded` from the old node into its freshly-built
    /// replacement, so a directory the user had opened stays open.
    pub fn set_filter(&mut self, filter: DirectoryFilter) {
        if self.config.filter == filter {
            return;
        }
        self.config.filter = filter;
        rebuild_from_cache(&mut self.root, &self.cache, filter);
        // Re-apply selection onto the new node graph. The selection
        // cursor (`self.selected_path`) is authoritative; the
        // per-node `is_selected` cache needs re-syncing after any
        // mutation that drops and recreates nodes.
        if let Some(p) = self.selected_path.clone() {
            self.sync_selection_flag(&p);
        }
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
    /// Selection is stored as a path cursor ([`DirectoryTree::selected_path`])
    /// on the widget, not on individual nodes, so this is O(1) and it
    /// survives both filter changes and ancestor collapse/expand cycles.
    /// The returned path may point to a node that is currently invisible
    /// (because an ancestor is collapsed, or because the active filter
    /// hides it); the view layer handles that gracefully.
    pub fn selected_path(&self) -> Option<&std::path::Path> {
        self.selected_path.as_deref()
    }

    /// Re-apply [`DirectoryTree::selected_path`] to the per-node
    /// `is_selected` flags used by the view.
    ///
    /// Called after any operation that may have replaced nodes
    /// (e.g. `set_filter`, a fresh `Loaded` payload arriving for a
    /// directory that contains the selected child).
    pub(crate) fn sync_selection_flag(&mut self, target: &std::path::Path) {
        self.root.clear_selection();
        if let Some(node) = self.root.find_mut(target) {
            node.is_selected = true;
        }
    }
}

/// Re-derive the `children` list at every already-loaded directory
/// in the tree from the unfiltered cache, applying `filter`.
///
/// Used by [`DirectoryTree::set_filter`] so a filter change is
/// instant. Unloaded directories are skipped — their filter will be
/// applied on first load, which is already correct without any help
/// from here.
///
/// Expansion state is preserved: before replacing a directory's
/// children we snapshot the `(path → is_expanded, is_loaded)` map of
/// the *old* children, then apply it to the *new* children built from
/// the raw cache. A directory the user had opened stays open, and a
/// grandchild already loaded stays loaded. Selection is re-applied
/// separately in [`DirectoryTree::set_filter`] via
/// [`DirectoryTree::sync_selection_flag`] because the selection
/// cursor lives on the widget, not on nodes.
fn rebuild_from_cache(node: &mut TreeNode, cache: &node::TreeCache, filter: DirectoryFilter) {
    if node.is_dir && node.is_loaded {
        if let Some(cached) = cache.get(&node.path) {
            // Snapshot old children by path so we can carry their
            // `is_expanded`, `is_loaded`, and transitive `children`
            // subtrees over. Without this, an ancestor's filter
            // change would wipe every descendant's loaded state —
            // even though none of the descendants' filesystem
            // listings actually changed.
            let mut previous: std::collections::HashMap<PathBuf, TreeNode> = node
                .children
                .drain(..)
                .map(|c| (c.path.clone(), c))
                .collect();
            node.children = cached
                .raw
                .iter()
                .filter(|e| e.passes(filter))
                .map(|e| {
                    // If this child already existed in the old tree,
                    // move it over wholesale — that preserves every
                    // flag and every deeper subtree in one step.
                    // Otherwise it's genuinely a new appearance (e.g.
                    // hidden → visible after flipping to
                    // AllIncludingHidden), so we build a fresh node.
                    previous
                        .remove(&e.path)
                        .unwrap_or_else(|| TreeNode::from_entry(e))
                })
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
