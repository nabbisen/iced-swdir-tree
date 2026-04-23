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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_files_and_folders() {
        assert_eq!(DirectoryFilter::default(), DirectoryFilter::FilesAndFolders);
    }

    #[test]
    fn filter_predicates() {
        assert!(DirectoryFilter::FoldersOnly.skips_hidden());
        assert!(DirectoryFilter::FoldersOnly.skips_files());

        assert!(DirectoryFilter::FilesAndFolders.skips_hidden());
        assert!(!DirectoryFilter::FilesAndFolders.skips_files());

        assert!(!DirectoryFilter::AllIncludingHidden.skips_hidden());
        assert!(!DirectoryFilter::AllIncludingHidden.skips_files());
    }
}
