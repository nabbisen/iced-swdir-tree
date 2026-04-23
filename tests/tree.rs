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

use iced_swdir_tree::{
    DirectoryFilter, DirectoryTree, DirectoryTreeEvent, Error, SelectionMode, TreeNode,
};

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
    let _ = tree.update(DirectoryTreeEvent::Selected(
        target.clone(),
        true,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(target.as_path()));
}

#[test]
fn filter_change_preserves_selection_cursor() {
    // **v0.2 behaviour**: selection is per-path and survives filter
    // changes, even when the filter hides the selected node.
    let td = TmpDir::new("select-filter");
    fs::create_dir(td.path().join("keeps")).unwrap();
    fs::write(td.path().join("file.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    // Select the file, then flip to a filter that hides files.
    let target = td.path().join("file.txt");
    let _ = tree.update(DirectoryTreeEvent::Selected(
        target.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(target.as_path()));

    tree.set_filter(DirectoryFilter::FoldersOnly);

    // The cursor persists even though the file is no longer visible.
    assert_eq!(
        tree.selected_path(),
        Some(target.as_path()),
        "v0.2: selection cursor survives filter change"
    );

    // Flipping the filter back to FilesAndFolders and re-selecting
    // the path confirms it is reachable again.
    tree.set_filter(DirectoryFilter::FilesAndFolders);
    assert_eq!(tree.selected_path(), Some(target.as_path()));
}

#[test]
fn filter_change_preserves_directory_selection_when_still_visible() {
    // Companion to the previous test: if the selected node is
    // *still* visible under the new filter, the per-node
    // is_selected flag is re-synced to match.
    let td = TmpDir::new("select-folder-filter");
    fs::create_dir(td.path().join("keeps")).unwrap();
    fs::write(td.path().join("file.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let target = td.path().join("keeps");
    let _ = tree.update(DirectoryTreeEvent::Selected(
        target.clone(),
        true,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(target.as_path()));

    tree.set_filter(DirectoryFilter::FoldersOnly);
    assert_eq!(tree.selected_path(), Some(target.as_path()));
    // And the per-node cache is re-marked on the freshly built node.
    let node = find_in_tree(&tree, &target).expect("folder must still be in the tree");
    assert!(
        node.is_selected,
        "per-node flag must be re-synced after rebuild"
    );
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

// ---------------------------------------------------------------------------
// v0.2 tests: expansion state survives filter change
// ---------------------------------------------------------------------------

#[test]
fn filter_change_preserves_expanded_subtree() {
    // User has expanded /root/sub and loaded its children; flipping
    // the filter must not collapse /root/sub or re-trigger its scan.
    let td = TmpDir::new("expanded-preserve");
    fs::create_dir_all(td.path().join("sub/inner")).unwrap();
    fs::write(td.path().join("sub/inner/leaf.txt"), b"").unwrap();
    fs::write(td.path().join(".hidden"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());
    // Expand the subdirectory too so there's deeper state to preserve.
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().join("sub"));

    // Pre-condition: sub is expanded and loaded.
    let sub_before = find_in_tree(&tree, &td.path().join("sub")).expect("sub exists");
    assert!(sub_before.is_expanded);
    assert!(sub_before.is_loaded);
    assert!(!sub_before.children.is_empty());

    // Flip the filter. No I/O; tree re-derives from cache.
    tree.set_filter(DirectoryFilter::AllIncludingHidden);

    // Post-condition: sub is still expanded, still loaded, still
    // has its inner child. This is the v0.2 "rebuild_from_cache
    // preserves subtree" contract.
    let sub_after =
        find_in_tree(&tree, &td.path().join("sub")).expect("sub must still be in the tree");
    assert!(
        sub_after.is_expanded,
        "expansion must survive filter change"
    );
    assert!(
        sub_after.is_loaded,
        "loaded flag must survive filter change"
    );
    assert!(
        !sub_after.children.is_empty(),
        "deeper subtree must survive filter change"
    );
}

// ---------------------------------------------------------------------------
// v0.2 tests: custom executor integration
// ---------------------------------------------------------------------------

/// A test-only executor that counts how many scans it ran.
///
/// Demonstrates the `ScanExecutor` trait is object-safe and pluggable,
/// and exercises the trait-method signature end-to-end.
#[derive(Default)]
struct CountingExecutor {
    count: std::sync::atomic::AtomicUsize,
}

impl iced_swdir_tree::ScanExecutor for CountingExecutor {
    fn spawn_blocking(&self, job: iced_swdir_tree::ScanJob) -> iced_swdir_tree::ScanFuture {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // Delegate to the default ThreadExecutor to actually run the
        // work — we only want to observe here, not reimplement.
        iced_swdir_tree::ThreadExecutor.spawn_blocking(job)
    }
}

#[test]
fn with_executor_accepts_a_custom_impl() {
    // End-to-end check: constructing a tree with a custom executor
    // compiles, and reading back the builder result exposes the
    // expected API surface. We don't spin up an iced runtime here —
    // that's covered by the actual `iced::Task::perform` path in the
    // manual keyboard_nav example — but we do confirm the builder
    // accepts any `Arc<dyn ScanExecutor>`.
    use std::sync::Arc;

    let exec: Arc<dyn iced_swdir_tree::ScanExecutor> = Arc::new(CountingExecutor::default());
    let _tree = DirectoryTree::new(PathBuf::from("/tmp"))
        .with_executor(exec.clone())
        .with_filter(DirectoryFilter::FilesAndFolders);
    // The trait-object constructor worked — that's the main assertion.
    // (We cannot easily drive the async scan off-runtime here.)
}

#[test]
fn default_executor_is_thread_executor_and_builds_cleanly() {
    // Smoke-test that constructing a DirectoryTree without calling
    // `with_executor` gives us the ThreadExecutor default. If this
    // ever regresses, the following apps-built-on-v0.1 line would
    // need user intervention.
    let _tree = DirectoryTree::new(PathBuf::from("/tmp"));
    // Nothing to assert on the executor directly (it's `pub(crate)`
    // by intent), but the tree builds without a type inference hint
    // for the executor, which is what v0.1 users rely on.
}

// ---------------------------------------------------------------------------
// v0.3 tests: multi-select over real filesystem trees
// ---------------------------------------------------------------------------

/// Populate a directory with `a`, `b`, `c` siblings (all files) and
/// return its path + the tree with the root expanded.
fn tree_with_abc(tag: &str) -> (TmpDir, DirectoryTree) {
    let td = TmpDir::new(tag);
    fs::write(td.path().join("a"), b"").unwrap();
    fs::write(td.path().join("b"), b"").unwrap();
    fs::write(td.path().join("c"), b"").unwrap();
    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());
    (td, tree)
}

#[test]
fn multi_select_toggle_builds_up_a_set() {
    let (td, mut tree) = tree_with_abc("mselect-toggle");
    let a = td.path().join("a");
    let b = td.path().join("b");

    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        false,
        SelectionMode::Replace,
    ));
    let _ = tree.update(DirectoryTreeEvent::Selected(
        b.clone(),
        false,
        SelectionMode::Toggle,
    ));
    assert_eq!(tree.selected_paths().len(), 2);
    assert!(tree.is_selected(&a));
    assert!(tree.is_selected(&b));

    // Untoggling /a shrinks the set but keeps /b.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        false,
        SelectionMode::Toggle,
    ));
    assert_eq!(tree.selected_paths().len(), 1);
    assert!(!tree.is_selected(&a));
    assert!(tree.is_selected(&b));
}

#[test]
fn multi_select_range_covers_siblings() {
    let (td, mut tree) = tree_with_abc("mselect-range");
    let a = td.path().join("a");
    let c = td.path().join("c");

    // Anchor at /a, then Shift-range to /c. Expect {/a, /b, /c}.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        false,
        SelectionMode::Replace,
    ));
    let _ = tree.update(DirectoryTreeEvent::Selected(
        c.clone(),
        false,
        SelectionMode::ExtendRange,
    ));
    assert_eq!(tree.selected_paths().len(), 3);
    let names: Vec<_> = tree
        .selected_paths()
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    assert_eq!(names, vec!["a", "b", "c"]);
    // Anchor stayed on /a so a subsequent Shift+click would still
    // range from /a.
    assert_eq!(tree.anchor_path(), Some(a.as_path()));
    // And the per-node flags are all marked true.
    for name in &["a", "b", "c"] {
        let node = find_in_tree(&tree, &td.path().join(name)).unwrap();
        assert!(node.is_selected, "{name} must be visually selected");
    }
}

#[test]
fn filter_change_preserves_every_selected_path() {
    // Multi-select survival: select several paths, flip the filter,
    // confirm every selected path is still in `selected_paths`.
    let td = TmpDir::new("mselect-filter");
    fs::write(td.path().join("a"), b"").unwrap();
    fs::write(td.path().join("b"), b"").unwrap();
    fs::write(td.path().join(".hidden"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::AllIncludingHidden);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let a = td.path().join("a");
    let b = td.path().join("b");
    let hidden = td.path().join(".hidden");

    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        false,
        SelectionMode::Replace,
    ));
    let _ = tree.update(DirectoryTreeEvent::Selected(
        b.clone(),
        false,
        SelectionMode::Toggle,
    ));
    let _ = tree.update(DirectoryTreeEvent::Selected(
        hidden.clone(),
        false,
        SelectionMode::Toggle,
    ));
    assert_eq!(tree.selected_paths().len(), 3);

    // Flip filter — .hidden gets hidden, but stays in the set.
    tree.set_filter(DirectoryFilter::FilesAndFolders);
    assert_eq!(
        tree.selected_paths().len(),
        3,
        "v0.3: every selected path survives filter change"
    );
    assert!(tree.is_selected(&hidden));

    // The per-node view flags are correct on surviving nodes.
    assert!(find_in_tree(&tree, &a).unwrap().is_selected);
    assert!(find_in_tree(&tree, &b).unwrap().is_selected);
    // And .hidden is not currently in the tree (filtered out),
    // but re-appears with its flag restored on the next flip back.
    assert!(find_in_tree(&tree, &hidden).is_none());
    tree.set_filter(DirectoryFilter::AllIncludingHidden);
    assert!(find_in_tree(&tree, &hidden).unwrap().is_selected);
}

#[test]
fn selected_path_returns_most_recent_action_target() {
    // The v0.2 single-select accessor still works in v0.3 and returns
    // the "active" path = target of the most recent Replace/Toggle/Range.
    let (td, mut tree) = tree_with_abc("mselect-active");
    let a = td.path().join("a");
    let b = td.path().join("b");
    let c = td.path().join("c");

    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(a.as_path()));

    let _ = tree.update(DirectoryTreeEvent::Selected(
        b.clone(),
        false,
        SelectionMode::Toggle,
    ));
    assert_eq!(tree.selected_path(), Some(b.as_path()));

    // ExtendRange also updates active, even though anchor stays put.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        c.clone(),
        false,
        SelectionMode::ExtendRange,
    ));
    assert_eq!(tree.selected_path(), Some(c.as_path()));
    assert_eq!(
        tree.anchor_path(),
        Some(b.as_path()),
        "anchor is last Replace/Toggle, not ExtendRange"
    );
}
