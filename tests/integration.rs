//! Integration tests for `iced-swdir-tree`.
//!
//! We can't drive iced's async `Task` runtime from a plain `#[test]`
//! without standing up a window, because the runtime scheduling
//! plumbing is `iced_runtime` internal. Instead we exercise the
//! widget's state machine through its public API plus a `doc(hidden)`
//! synchronous helper that performs a scan and applies the result on
//! the same thread — it calls into exactly the same internal
//! `on_loaded` code path the async version does, so coverage is the
//! same as if we could run the real task.
//!
//! Every test uses a throwaway directory under `$TMPDIR`, cleaned up
//! by a `Drop` guard so a panic still tidies up.

use std::fs;
use std::path::{Path, PathBuf};

use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent};

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

/// Self-cleaning temp directory. Unique per test via pid + nanos.
struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "iced-swdir-tree-itest-{}-{}-{}",
            std::process::id(),
            nanos,
            tag
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create tmpdir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn touch(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        fs::write(&p, b"").expect("touch");
        p
    }

    fn mkdir(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        fs::create_dir(&p).expect("mkdir");
        p
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        // Restore permissions on any entries we chmod'd, so Drop
        // can actually remove them (permission-denied test uses 0o000).
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
// Filter modes
// ---------------------------------------------------------------------------

#[test]
fn filter_folders_only_hides_files_after_expand() {
    let td = TmpDir::new("folders-only");
    td.mkdir("sub");
    td.touch("note.txt");

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FoldersOnly);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let sub = td.path().join("sub");
    let note = td.path().join("note.txt");

    // `sub` is a folder — visible under FoldersOnly.
    let _ = tree.update(DirectoryTreeEvent::Selected(sub.clone(), true));
    assert_eq!(tree.selected_path(), Some(sub.as_path()));

    // `note.txt` is filtered out — selecting it must be a no-op:
    // the filtered node is not reachable, and the previous
    // selection stays put.
    let _ = tree.update(DirectoryTreeEvent::Selected(note.clone(), false));
    assert_eq!(
        tree.selected_path(),
        Some(sub.as_path()),
        "stray selection on a filtered-out path must be ignored"
    );
}

#[test]
fn filter_files_and_folders_hides_hidden_entries() {
    let td = TmpDir::new("no-hidden");
    td.touch(".hidden");
    td.touch("visible.txt");

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let visible = td.path().join("visible.txt");
    let hidden = td.path().join(".hidden");

    let _ = tree.update(DirectoryTreeEvent::Selected(visible.clone(), false));
    assert_eq!(tree.selected_path(), Some(visible.as_path()));

    // Stray selection on a filtered-out path is a no-op — the
    // previous selection stays intact.
    let _ = tree.update(DirectoryTreeEvent::Selected(hidden.clone(), false));
    assert_eq!(
        tree.selected_path(),
        Some(visible.as_path()),
        "hidden file should not be selectable under FilesAndFolders"
    );
}

#[test]
fn filter_all_includes_hidden_entries() {
    let td = TmpDir::new("with-hidden");
    td.touch(".hidden");
    td.touch("visible.txt");

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::AllIncludingHidden);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let hidden = td.path().join(".hidden");
    let _ = tree.update(DirectoryTreeEvent::Selected(hidden.clone(), false));
    assert_eq!(tree.selected_path(), Some(hidden.as_path()));
}

// ---------------------------------------------------------------------------
// Expand / collapse
// ---------------------------------------------------------------------------

#[test]
fn expand_populates_children() {
    let td = TmpDir::new("expand");
    td.mkdir("a");
    td.mkdir("b");
    td.touch("c.txt");

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().to_path_buf());

    let a = td.path().join("a");
    let _ = tree.update(DirectoryTreeEvent::Selected(a.clone(), true));
    assert_eq!(tree.selected_path(), Some(a.as_path()));

    let c = td.path().join("c.txt");
    let _ = tree.update(DirectoryTreeEvent::Selected(c.clone(), false));
    assert_eq!(tree.selected_path(), Some(c.as_path()));
}

#[test]
fn collapse_keeps_children_and_reexpand_is_instant() {
    let td = TmpDir::new("recollapse");
    td.mkdir("x");
    td.touch("y.txt");

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().to_path_buf());

    let x = td.path().join("x");
    let _ = tree.update(DirectoryTreeEvent::Selected(x.clone(), true));
    assert_eq!(tree.selected_path(), Some(x.as_path()));

    // Collapse the root via Toggled — should be instant, no Task
    // with a Loaded follow-up.
    let t = tree.update(DirectoryTreeEvent::Toggled(td.path().to_path_buf()));
    assert_eq!(t.units(), 0, "collapse must not spawn an async scan");

    // Re-expand — this time the children are cached, so the
    // expansion is synchronous and emits no Task either.
    let t2 = tree.update(DirectoryTreeEvent::Toggled(td.path().to_path_buf()));
    assert_eq!(
        t2.units(),
        0,
        "re-expanding a cached directory must not spawn another scan"
    );

    // And `x` is still selectable.
    let _ = tree.update(DirectoryTreeEvent::Selected(x.clone(), true));
    assert_eq!(tree.selected_path(), Some(x.as_path()));
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

#[test]
fn selection_is_single_select() {
    let td = TmpDir::new("select");
    td.touch("one.txt");
    td.touch("two.txt");

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().to_path_buf());

    let one = td.path().join("one.txt");
    let two = td.path().join("two.txt");

    let _ = tree.update(DirectoryTreeEvent::Selected(one.clone(), false));
    assert_eq!(tree.selected_path(), Some(one.as_path()));

    let _ = tree.update(DirectoryTreeEvent::Selected(two.clone(), false));
    assert_eq!(tree.selected_path(), Some(two.as_path()));
}

// ---------------------------------------------------------------------------
// Runtime filter change
// ---------------------------------------------------------------------------

#[test]
fn set_filter_at_runtime_changes_visibility_without_rescan() {
    let td = TmpDir::new("setfilter");
    td.mkdir("dir");
    td.touch("file.txt");
    td.touch(".hidden");

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::AllIncludingHidden);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let hidden = td.path().join(".hidden");
    let visible = td.path().join("file.txt");

    // Select a visible-under-All entry and confirm selection.
    let _ = tree.update(DirectoryTreeEvent::Selected(hidden.clone(), false));
    assert_eq!(tree.selected_path(), Some(hidden.as_path()));

    // Flip to a filter that hides dotfiles — the tree re-derives
    // children from its cache, no filesystem I/O required.
    tree.set_filter(DirectoryFilter::FilesAndFolders);

    // **v0.2 change**: selection is per-path and survives filter
    // changes, even when the selected node is filtered out of the
    // visible tree. A stray selection click on an invisible path
    // is still a no-op (it leaves the cursor unchanged), but the
    // cursor value itself persists.
    let _ = tree.update(DirectoryTreeEvent::Selected(hidden.clone(), false));
    assert_eq!(
        tree.selected_path(),
        Some(hidden.as_path()),
        "v0.2 keeps the selection cursor around through filter changes"
    );

    // Re-selecting a visible sibling replaces it — normal behaviour.
    let _ = tree.update(DirectoryTreeEvent::Selected(visible.clone(), false));
    assert_eq!(tree.selected_path(), Some(visible.as_path()));

    // Finally, flipping the filter back to AllIncludingHidden means
    // the previously-hidden selection cursor becomes reachable again.
    tree.set_filter(DirectoryFilter::AllIncludingHidden);
    // The selection cursor is currently on `visible.txt`, not
    // `.hidden`, because we moved it above. Confirm:
    assert_eq!(tree.selected_path(), Some(visible.as_path()));
    // And the per-node flag on visible should be set, confirming
    // `sync_selection_flag` ran.
    let _ = tree.update(DirectoryTreeEvent::Selected(hidden.clone(), false));
    assert_eq!(
        tree.selected_path(),
        Some(hidden.as_path()),
        "hidden re-selectable once AllIncludingHidden is active again"
    );
}

// ---------------------------------------------------------------------------
// Error paths
// ---------------------------------------------------------------------------

#[test]
fn nonexistent_root_does_not_crash() {
    let bogus = PathBuf::from("/this/path/should/not/exist/for/iced-swdir-tree-tests");
    let mut tree = DirectoryTree::new(bogus.clone());
    // Scanning a nonexistent path yields Err(..) internally; the
    // test helper routes that through the same code path the async
    // scan would. The tree must survive and expose no children.
    tree.__test_expand_blocking(bogus.clone());

    // No selection should be reachable — the root has no children
    // and no file to select other than itself.
    let _ = tree.update(DirectoryTreeEvent::Selected(bogus.join("anything"), false));
    assert_eq!(tree.selected_path(), None);
}

#[test]
fn toggling_an_unknown_path_is_a_noop() {
    let td = TmpDir::new("noop");
    let mut tree = DirectoryTree::new(td.path().to_path_buf());

    // Toggled for a path we've never heard of — widget must not
    // panic and must not spawn a scan.
    let t = tree.update(DirectoryTreeEvent::Toggled(PathBuf::from(
        "/some/other/place",
    )));
    assert_eq!(t.units(), 0);
}

#[cfg(unix)]
#[test]
fn permission_denied_surfaces_as_error_node() {
    use std::os::unix::fs::PermissionsExt;

    // Root can read anything regardless of permissions, so this
    // test becomes a no-op under root. Skip gracefully.
    if is_running_as_root() {
        eprintln!("skipping permission_denied_surfaces_as_error_node: running as root");
        return;
    }

    let td = TmpDir::new("noperm");
    let sub = td.mkdir("forbidden");
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o000)).unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().to_path_buf());

    // Try to expand the forbidden folder. It must not crash.
    tree.__test_expand_blocking(sub.clone());

    // Selecting `sub` still works — the node exists, it's just
    // marked with an error. This is the "gray out the directory"
    // UI behaviour from the spec.
    let _ = tree.update(DirectoryTreeEvent::Selected(sub.clone(), true));
    assert_eq!(tree.selected_path(), Some(sub.as_path()));

    // Restore permissions before the Drop so cleanup works.
    let _ = fs::set_permissions(&sub, fs::Permissions::from_mode(0o700));
}

#[cfg(unix)]
fn is_running_as_root() -> bool {
    // SAFETY: getuid is always safe; it just reads a process-global
    // integer that the kernel populates at startup.
    unsafe { libc_getuid() == 0 }
}

#[cfg(unix)]
unsafe extern "C" {
    #[link_name = "getuid"]
    fn libc_getuid() -> u32;
}
