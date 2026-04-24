//! v0.5 integration: parallel pre-expansion of visible descendants.
//!
//! The `__test_expand_blocking` helper simulates the full
//! `Toggled → scan → Loaded → prefetch-scans → Loaded` round-trip
//! synchronously, so integration tests can assert on the tree's
//! state after a user expansion as if the iced runtime had driven
//! the prefetch tasks to completion.

mod common;
use common::{TmpDir, find_in_tree};

use std::fs;

use iced_swdir_tree::{DirectoryFilter, DirectoryTree};

/// Baseline: without prefetch, only the root's direct children are
/// loaded after expanding the root. Folder-children remain
/// `is_loaded = false` until the user clicks them.
#[test]
fn prefetch_disabled_keeps_child_folders_unloaded() {
    let td = TmpDir::new("prefetch-off");
    fs::create_dir(td.path().join("a")).unwrap();
    fs::create_dir(td.path().join("b")).unwrap();
    fs::write(td.path().join("a/inside.txt"), b"").unwrap();
    fs::write(td.path().join("b/inside.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());

    for name in &["a", "b"] {
        let node = find_in_tree(&tree, &td.path().join(name)).expect("child exists");
        assert!(node.is_dir);
        assert!(
            !node.is_loaded,
            "{name}: prefetch is disabled, so child folder must not \
             have been pre-scanned"
        );
        assert!(node.children.is_empty());
    }
}

/// With `with_prefetch_limit(10)`, after expanding the root, all
/// folder-children show `is_loaded = true` and have populated
/// `children` — even though the user has clicked only once.
#[test]
fn prefetch_loads_child_folders_after_root_expansion() {
    let td = TmpDir::new("prefetch-on");
    fs::create_dir(td.path().join("alpha")).unwrap();
    fs::create_dir(td.path().join("beta")).unwrap();
    fs::write(td.path().join("alpha/x.txt"), b"").unwrap();
    fs::write(td.path().join("beta/y.txt"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::FilesAndFolders)
        .with_prefetch_limit(10);
    tree.__test_expand_blocking(td.path().to_path_buf());

    for name in &["alpha", "beta"] {
        let node = find_in_tree(&tree, &td.path().join(name)).expect("child exists");
        assert!(
            node.is_loaded,
            "{name}: prefetch should have populated this folder"
        );
        assert_eq!(
            node.children.len(),
            1,
            "{name}: prefetched children should include its own file"
        );
    }
}

/// Prefetch is **one level deep only** — a folder that was loaded
/// via prefetch does not itself trigger further prefetches. So
/// grand-children remain unloaded.
#[test]
fn prefetch_does_not_cascade() {
    let td = TmpDir::new("prefetch-nocascade");
    fs::create_dir_all(td.path().join("outer/inner")).unwrap();
    fs::write(td.path().join("outer/inner/deep.txt"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::FilesAndFolders)
        .with_prefetch_limit(5);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let outer = find_in_tree(&tree, &td.path().join("outer")).expect("outer exists");
    assert!(outer.is_loaded, "outer should be prefetched");
    // outer was prefetched, so its child `inner` must NOT also be
    // prefetched. The cascade is explicitly blocked.
    let inner_node = outer
        .children
        .iter()
        .find(|c| c.path.file_name().is_some_and(|n| n == "inner"))
        .expect("inner must appear as a child of outer");
    assert!(
        !inner_node.is_loaded,
        "cascade: inner must NOT have been prefetched by outer's prefetch"
    );
}

/// Prefetch respects `with_max_depth`: children past the cap are
/// skipped rather than scanned.
#[test]
fn prefetch_respects_max_depth() {
    let td = TmpDir::new("prefetch-depth");
    fs::create_dir(td.path().join("a")).unwrap();
    fs::write(td.path().join("a/file"), b"").unwrap();

    // max_depth=0 forbids any descent into a/ (a/ is at depth 1).
    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::FilesAndFolders)
        .with_max_depth(0)
        .with_prefetch_limit(5);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let a = find_in_tree(&tree, &td.path().join("a")).expect("child exists");
    assert!(
        !a.is_loaded,
        "max_depth=0 must prevent prefetch from scanning a/"
    );
}

/// With `with_prefetch_limit(1)`, only the *first* folder-child
/// gets prefetched. Later ones stay unloaded.
#[test]
fn prefetch_limit_caps_the_number_of_concurrent_scans() {
    let td = TmpDir::new("prefetch-limit");
    // Folders sort alphabetically, so `aa` is first, `zz` last.
    for name in &["aa", "mm", "zz"] {
        fs::create_dir(td.path().join(name)).unwrap();
        fs::write(td.path().join(name).join("payload"), b"").unwrap();
    }

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::FilesAndFolders)
        .with_prefetch_limit(1);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let aa = find_in_tree(&tree, &td.path().join("aa")).unwrap();
    assert!(
        aa.is_loaded,
        "first folder in sort order should be prefetched"
    );
    let zz = find_in_tree(&tree, &td.path().join("zz")).unwrap();
    assert!(
        !zz.is_loaded,
        "with limit=1, only the first folder is prefetched; \
         later ones stay un-loaded until the user opens them"
    );
}

/// Clicking to expand a prefetched folder is instant: the cache is
/// already populated, no further scan is dispatched. The same
/// `is_expanded → is_loaded → no-op` fast path that v0.1–0.4 had
/// for *re*-opening a folder is now hit on the *first* open too.
#[test]
fn clicking_a_prefetched_folder_is_instant() {
    use iced_swdir_tree::DirectoryTreeEvent;

    let td = TmpDir::new("prefetch-fastpath");
    fs::create_dir(td.path().join("ready")).unwrap();
    fs::write(td.path().join("ready/file"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::FilesAndFolders)
        .with_prefetch_limit(10);
    tree.__test_expand_blocking(td.path().to_path_buf());

    // `ready` was prefetched; now simulate the user clicking its
    // expand toggle. The returned Task must be empty — zero async
    // work, because `is_loaded` is already true.
    let ready = td.path().join("ready");
    let task = tree.update(DirectoryTreeEvent::Toggled(ready.clone()));
    assert_eq!(
        task.units(),
        0,
        "expanding a prefetched folder must not spawn a new scan"
    );
    let node = find_in_tree(&tree, &ready).unwrap();
    assert!(node.is_expanded);
    assert!(!node.children.is_empty());
}
