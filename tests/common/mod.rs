//! Shared fixtures and helpers for integration tests.
//!
//! Rust treats every `.rs` file directly under `tests/` as an
//! independent test binary, but any subdirectory (like this one) is
//! NOT compiled on its own — it's shared code that each top-level
//! test file pulls in via `mod common;`. See
//! <https://doc.rust-lang.org/cargo/guide/tests.html#the-tests-directory>.
//!
//! Not every test uses every helper here, so `#[allow(dead_code)]`
//! is deliberate — Rust's unused-item lint fires per-compilation-unit
//! (i.e., per test binary), and an item that only `filter_modes.rs`
//! uses is "dead" from `tree_drag_drop.rs`'s perspective.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use iced_swdir_tree::{DirectoryTree, TreeNode};

// ---------------------------------------------------------------------------
// Temp-directory fixture
// ---------------------------------------------------------------------------

/// Self-cleaning temp directory, namespaced with pid + nanos + tag so
/// cargo's parallel test runner can't collide.
pub struct TmpDir(pub PathBuf);

impl TmpDir {
    pub fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p = std::env::temp_dir().join(format!(
            "iced-swdir-tree-it-{}-{}-{}",
            std::process::id(),
            nanos,
            tag
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("create tmpdir");
        Self(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Create an empty file in the tmp root and return its path.
    pub fn touch(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        fs::write(&p, b"").expect("touch");
        p
    }

    /// Create a subdirectory in the tmp root and return its path.
    pub fn mkdir(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        fs::create_dir(&p).expect("mkdir");
        p
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        // Restore permissions on any entries we chmod'd, so Drop
        // can actually remove them (permission-denied tests use 0o000).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(entries) = fs::read_dir(&self.0) {
                for e in entries.flatten() {
                    let _ = fs::set_permissions(e.path(), fs::Permissions::from_mode(0o700));
                }
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Tree introspection helpers
// ---------------------------------------------------------------------------

/// Collect the root's direct children file names into a Vec.
pub fn child_names(tree: &DirectoryTree) -> Vec<String> {
    let root = find_in_tree(tree, tree.root_path()).expect("root exists");
    root.children
        .iter()
        .map(|n| {
            n.path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect()
}

/// Locate a node by path via read-only recursion.
pub fn find_in_tree<'a>(tree: &'a DirectoryTree, path: &Path) -> Option<&'a TreeNode> {
    fn walk<'a>(node: &'a TreeNode, path: &Path) -> Option<&'a TreeNode> {
        if node.path == path {
            return Some(node);
        }
        if !path.starts_with(&node.path) {
            return None;
        }
        node.children.iter().find_map(|c| walk(c, path))
    }
    walk(iced_swdir_tree::__testing::root(tree), path)
}

/// Are we running as root / euid 0?
///
/// Used to skip the permission-denied tests when they'd be
/// meaningless (root bypasses DAC_READ_SEARCH). Probed via
/// `/proc/self/status` rather than libc so we don't pull in a dep
/// for one syscall.
pub fn is_root() -> bool {
    fs::read_to_string("/proc/self/status")
        .map(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Uid:"))
                .map(|rest| rest.split_whitespace().next().unwrap_or("1000") == "0")
                .unwrap_or(false)
        })
        .unwrap_or(false)
}
