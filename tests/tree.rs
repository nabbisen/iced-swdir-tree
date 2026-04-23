//! Integration tests covering the full `DirectoryTree` state machine
//! against a real temporary filesystem.
//!
//! These tests deliberately do not spin up an iced executor — running
//! one headlessly is overkill for state-machine coverage. Instead,
//! they drive the widget through the crate's `__testing` shim, which
//! reuses the exact normalization path the async task uses in
//! production (minus the thread hop).

use std::fs;
use std::path::{Path, PathBuf};

use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent, Error, TreeNode};

// ---------------------------------------------------------------------------
// Temp-directory fixture
// ---------------------------------------------------------------------------

/// Self-cleaning temp directory, namespaced with pid + nanos + tag so
/// cargo's parallel test runner can't collide.
struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
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

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn files_and_folders_filter_shows_both() {
    let td = TmpDir::new("fandf");
    fs::create_dir(td.path().join("sub")).unwrap();
    fs::write(td.path().join("visible.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let names = child_names(&tree);
    assert!(names.contains(&"sub".into()), "folder must be listed");
    assert!(names.contains(&"visible.txt".into()), "file must be listed");
    assert_eq!(names.len(), 2);
}

#[test]
fn folders_only_filter_drops_files() {
    let td = TmpDir::new("fonly");
    fs::create_dir(td.path().join("sub")).unwrap();
    fs::write(td.path().join("file.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FoldersOnly);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let names = child_names(&tree);
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "sub");
}

#[test]
fn all_including_hidden_shows_dotfiles() {
    let td = TmpDir::new("hidden");
    fs::write(td.path().join(".secret"), b"").unwrap();
    fs::write(td.path().join("visible.txt"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::AllIncludingHidden);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let names = child_names(&tree);
    assert_eq!(names.len(), 2);
    assert!(names.contains(&".secret".into()));
}

#[test]
fn default_filter_hides_dotfiles() {
    let td = TmpDir::new("no-dot");
    fs::write(td.path().join(".secret"), b"").unwrap();
    fs::write(td.path().join("visible.txt"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let names = child_names(&tree);
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "visible.txt");
}

#[test]
fn filter_change_rebuilds_from_cache() {
    let td = TmpDir::new("refilter");
    fs::create_dir(td.path().join("sub")).unwrap();
    fs::write(td.path().join(".secret"), b"").unwrap();
    fs::write(td.path().join("visible.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());
    assert_eq!(child_names(&tree).len(), 2);

    // Flip to AllIncludingHidden — must add `.secret` in place
    // without issuing another scan.
    tree.set_filter(DirectoryFilter::AllIncludingHidden);
    assert_eq!(child_names(&tree).len(), 3);
    assert!(child_names(&tree).contains(&".secret".into()));

    // Flip to FoldersOnly — every file disappears.
    tree.set_filter(DirectoryFilter::FoldersOnly);
    let names = child_names(&tree);
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "sub");
}

#[test]
fn collapse_then_reexpand_keeps_children() {
    let td = TmpDir::new("collapse");
    fs::create_dir(td.path().join("a")).unwrap();
    fs::write(td.path().join("b.txt"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());
    assert_eq!(child_names(&tree).len(), 2);

    // Collapse the root. Children must remain (collapse is instant,
    // no I/O).
    let _ = tree.update(DirectoryTreeEvent::Toggled(td.path().to_path_buf()));
    // Re-expand. Must not re-issue a scan (is_loaded stays true);
    // children should still be the same list.
    let _ = tree.update(DirectoryTreeEvent::Toggled(td.path().to_path_buf()));
    assert_eq!(child_names(&tree).len(), 2);
}

#[test]
fn selection_lands_on_the_target_path() {
    let td = TmpDir::new("select");
    fs::create_dir(td.path().join("keeps")).unwrap();
    fs::create_dir(td.path().join("also")).unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let target = td.path().join("keeps");
    let _ = tree.update(DirectoryTreeEvent::Selected(target.clone(), true));
    assert_eq!(tree.selected_path(), Some(target.as_path()));
}

#[test]
fn filter_change_drops_child_selection_as_documented() {
    // v0.1 documented simplification: rebuild_from_cache discards
    // per-node expansion/selection state.
    let td = TmpDir::new("select-filter");
    fs::create_dir(td.path().join("keeps")).unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let target = td.path().join("keeps");
    let _ = tree.update(DirectoryTreeEvent::Selected(target.clone(), true));
    assert_eq!(tree.selected_path(), Some(target.as_path()));

    tree.set_filter(DirectoryFilter::FoldersOnly);
    assert_eq!(tree.selected_path(), None);
}

#[test]
fn nonexistent_path_surfaces_as_error_not_panic() {
    let missing = std::env::temp_dir().join(format!(
        "iced-swdir-tree-nothing-here-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = fs::remove_dir_all(&missing);
    let mut tree = DirectoryTree::new(missing.clone());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, missing.clone());

    let root = find_in_tree(&tree, &missing).expect("root always present");
    assert!(
        root.error.is_some(),
        "missing path must surface as node.error, not a panic or empty list"
    );
    // And a specific ErrorKind so downstream apps can branch on it.
    if let Some(Error::Io { kind, .. }) = root.error.clone() {
        assert_eq!(kind, std::io::ErrorKind::NotFound);
    } else {
        panic!("expected Error::Io");
    }
}

#[cfg(unix)]
#[test]
fn permission_denied_is_greyed_out_not_fatal() {
    use std::os::unix::fs::PermissionsExt;

    // Skip the test if running as root — chmod 0o000 is bypassed by
    // CAP_DAC_OVERRIDE, which most CI runners have. Rootless runs
    // still exercise the code path.
    if is_root() {
        eprintln!("skipping permission test under root (CAP_DAC_OVERRIDE bypasses chmod)");
        return;
    }

    let td = TmpDir::new("perm");
    let locked = td.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    // Restore perms before the temp dir is dropped, so cleanup works.
    struct Restorer<'a>(&'a Path);
    impl Drop for Restorer<'_> {
        fn drop(&mut self) {
            let _ = fs::set_permissions(self.0, fs::Permissions::from_mode(0o755));
        }
    }
    let _restorer = Restorer(&locked);

    let mut tree = DirectoryTree::new(locked.clone());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, locked.clone());

    let node = find_in_tree(&tree, &locked).expect("locked dir tracked");
    assert!(
        node.error.is_some(),
        "permission denied must surface as error"
    );
    if let Some(Error::Io { kind, .. }) = node.error.clone() {
        assert_eq!(kind, std::io::ErrorKind::PermissionDenied);
    } else {
        panic!("expected Error::Io, got {:?}", node.error);
    }
}

#[cfg(unix)]
fn is_root() -> bool {
    // SAFETY: libc::geteuid is a pure syscall wrapper with no
    // preconditions. We avoid pulling in the `libc` crate for one
    // call by going through /proc/self/status, which is more portable
    // across containers.
    fs::read_to_string("/proc/self/status")
        .map(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Uid:"))
                .map(|rest| rest.split_whitespace().next().unwrap_or("1000") == "0")
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Collect the root's direct children file names into a Vec.
fn child_names(tree: &DirectoryTree) -> Vec<String> {
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
fn find_in_tree<'a>(tree: &'a DirectoryTree, path: &Path) -> Option<&'a TreeNode> {
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
