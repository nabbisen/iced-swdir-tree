//! Configuration types: [`DirectoryFilter`] and [`TreeConfig`].

use std::path::PathBuf;

/// Controls which entries the widget displays.
///
/// The widget *always* scans every entry of an expanded directory
/// (swdir's `scan_dir` makes no filtering decisions); the filter is
/// applied as we normalize raw entries into [`TreeNode`]s. That means
/// a filter change takes effect on the next view without needing to
/// re-scan the filesystem.
///
/// [`TreeNode`]: crate::TreeNode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DirectoryFilter {
    /// Show only directories (convenient for "pick a destination folder"
    /// pickers).
    FoldersOnly,
    /// Show both files and directories, but skip hidden entries (the
    /// default — matches most OS file pickers).
    #[default]
    FilesAndFolders,
    /// Show everything, including hidden entries.
    AllIncludingHidden,
}

impl DirectoryFilter {
    /// `true` if the filter suppresses hidden entries.
    pub fn skips_hidden(self) -> bool {
        !matches!(self, Self::AllIncludingHidden)
    }

    /// `true` if the filter suppresses regular files.
    pub fn skips_files(self) -> bool {
        matches!(self, Self::FoldersOnly)
    }
}

/// Per-tree configuration.
///
/// Constructed internally by [`DirectoryTree::new`] and its builder
/// methods; exposed as `pub` so tests and downstream tooling can
/// introspect the configuration.
///
/// [`DirectoryTree::new`]: crate::DirectoryTree::new
#[derive(Debug, Clone)]
pub struct TreeConfig {
    /// The tree's root directory.
    pub root_path: PathBuf,
    /// Active display filter.
    pub filter: DirectoryFilter,
    /// Maximum depth to descend into. `None` = unbounded.
    ///
    /// Depth is measured relative to the root: `Some(0)` means only
    /// root's direct children load, `Some(1)` allows grandchildren,
    /// and so on.
    pub max_depth: Option<u32>,
    /// **v0.5 — parallel pre-expansion of visible descendants.**
    ///
    /// When a user-initiated expansion finishes loading a folder,
    /// eagerly issue background scans for up to this many of the
    /// folder's direct children-that-are-folders, in parallel via
    /// [`ScanExecutor`]. The scans populate the in-memory cache
    /// (`is_loaded = true`) but do **not** automatically expand the
    /// children in the UI — the user still controls what's visible.
    /// When they later click to expand one of those children, the
    /// data is already there: no I/O, no thread spawn, no delay.
    ///
    /// `0` (the default) disables prefetch entirely, matching v0.1–0.4
    /// behaviour exactly. Higher values improve perceived
    /// responsiveness at the cost of background I/O on every
    /// user-initiated expansion. Typical app values: `5`–`25`. A huge
    /// value just means "prefetch every child folder"; the crate
    /// doesn't cap it because apps with fast executors may legitimately
    /// want that.
    ///
    /// Prefetch is **one level deep only** — a folder that loaded via
    /// prefetch does not itself trigger further prefetches of its
    /// children. This is intentional: cascading prefetch is
    /// exponential (`per_parent ^ depth`) and would be surprising as
    /// a default. If you need deeper prefetch, issue further
    /// [`DirectoryTreeEvent::Toggled`](crate::DirectoryTreeEvent::Toggled)
    /// events yourself from your app's update handler, or keep an
    /// eye on a future release — deeper cascade behind an opt-in may
    /// be added in a patch.
    ///
    /// Prefetch respects `max_depth` the same way user-initiated
    /// scans do: a prefetch target past the depth cap is skipped.
    ///
    /// [`ScanExecutor`]: crate::ScanExecutor
    pub prefetch_per_parent: usize,
}

#[cfg(test)]
mod tests;
