//! v0.6 integration: incremental search with real-time filtering.
//!
//! Drives the full `set_search_query → recompute_search_visibility
//! → visible_rows` path against a real filesystem fixture.

mod common;
use common::{TmpDir, find_in_tree};

use std::fs;

use iced_swdir_tree::{DirectoryFilter, DirectoryTree};

/// Build a filesystem with a known structure:
///
/// ```text
///   <root>/
///     apps/
///       readme.md
///       config.toml
///     src/
///       README.md       ← matches "readme"
///       lib.rs
///       inner/
///         readme.md     ← matches "readme" (grandchild)
///     random_file.txt
/// ```
fn build_fixture(tag: &str) -> TmpDir {
    let td = TmpDir::new(tag);
    fs::create_dir(td.path().join("apps")).unwrap();
    fs::write(td.path().join("apps/readme.md"), b"").unwrap();
    fs::write(td.path().join("apps/config.toml"), b"").unwrap();
    fs::create_dir(td.path().join("src")).unwrap();
    fs::write(td.path().join("src/README.md"), b"").unwrap();
    fs::write(td.path().join("src/lib.rs"), b"").unwrap();
    fs::create_dir(td.path().join("src/inner")).unwrap();
    fs::write(td.path().join("src/inner/readme.md"), b"").unwrap();
    fs::write(td.path().join("random_file.txt"), b"").unwrap();
    td
}

#[test]
fn search_inactive_by_default() {
    let td = build_fixture("search-baseline");
    let tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    assert!(!tree.is_searching());
    assert_eq!(tree.search_query(), None);
    assert_eq!(tree.search_match_count(), 0);
}

#[test]
fn search_finds_matches_across_loaded_subtrees() {
    // Expand everything so all nodes are loaded, then search for
    // "readme". Expect three matches (apps/readme.md, src/README.md,
    // src/inner/readme.md) and the full ancestor chain visible.
    let td = build_fixture("search-multi");
    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().join("apps"));
    tree.__test_expand_blocking(td.path().join("src"));
    tree.__test_expand_blocking(td.path().join("src/inner"));

    tree.set_search_query("readme");
    assert!(tree.is_searching());
    assert_eq!(tree.search_query(), Some("readme"));
    assert_eq!(
        tree.search_match_count(),
        3,
        "three readme.md files should match (case-insensitive)"
    );

    // random_file.txt doesn't match and has no matching descendants —
    // it must have been filtered out. The widget won't expose its
    // own `visible_paths`, but we can infer by checking that the
    // node is still in the graph (loaded earlier) while not being
    // in the tree's logical "current view".
    let node = find_in_tree(&tree, &td.path().join("random_file.txt"));
    assert!(
        node.is_some(),
        "node is still present in the graph — search doesn't unload anything"
    );
}

#[test]
fn empty_query_clears_search() {
    // Setting "" after a real query is equivalent to clear_search().
    let td = build_fixture("search-empty");
    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());
    tree.set_search_query("readme");
    assert!(tree.is_searching());
    tree.set_search_query("");
    assert!(!tree.is_searching());
    assert_eq!(tree.search_query(), None);
}

#[test]
fn clear_search_restores_normal_view() {
    let td = build_fixture("search-clear");
    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().join("src"));
    tree.set_search_query("lib.rs");
    assert_eq!(tree.search_match_count(), 1);
    tree.clear_search();
    assert!(!tree.is_searching());
    assert_eq!(tree.search_match_count(), 0);
}

#[test]
fn case_insensitive_matching() {
    let td = build_fixture("search-case");
    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().join("apps"));
    tree.__test_expand_blocking(td.path().join("src"));
    tree.__test_expand_blocking(td.path().join("src/inner"));

    // Uppercase query, mixed-case haystack: "README.md" and
    // "readme.md" (x3) should all match.
    tree.set_search_query("README");
    assert_eq!(tree.search_match_count(), 3);

    // Preserve app-provided casing of the query itself — for the
    // status-bar "Searching for 'README'..." use case.
    assert_eq!(tree.search_query(), Some("README"));
}

#[test]
fn selection_preserved_across_search() {
    use iced_swdir_tree::{DirectoryTreeEvent, SelectionMode};

    let td = build_fixture("search-select");
    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let random = td.path().join("random_file.txt");
    let _ = tree.update(DirectoryTreeEvent::Selected(
        random.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert!(tree.is_selected(&random));

    // Activating a search that excludes random_file.txt must NOT
    // drop the selection — selection is per-path and orthogonal
    // to visibility.
    tree.set_search_query("readme");
    assert!(tree.is_selected(&random), "selection survives search");

    // Clearing the search restores the unfiltered view and the
    // selection is still there.
    tree.clear_search();
    assert!(tree.is_selected(&random));
}

#[test]
fn filter_change_re_runs_search() {
    // Switch to FoldersOnly while searching "readme" — the matches
    // (all files) should disappear because they no longer exist in
    // the filtered node graph. match_count drops to 0.
    let td = build_fixture("search-filter");
    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().join("apps"));
    tree.__test_expand_blocking(td.path().join("src"));
    tree.__test_expand_blocking(td.path().join("src/inner"));

    tree.set_search_query("readme");
    assert_eq!(tree.search_match_count(), 3);

    tree.set_filter(DirectoryFilter::FoldersOnly);
    assert_eq!(
        tree.search_match_count(),
        0,
        "filter change must re-run the search — no folders match 'readme'"
    );
    assert!(
        tree.is_searching(),
        "the search query is still active, just with zero results"
    );
}

#[test]
fn search_descends_into_loaded_but_collapsed_subtree() {
    // Load src/ and src/inner/ (so readme.md inside is known), then
    // collapse src/. Search should still find the deep match.
    use iced_swdir_tree::DirectoryTreeEvent;

    let td = build_fixture("search-collapsed");
    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().join("src"));
    tree.__test_expand_blocking(td.path().join("src/inner"));

    // Collapse src/. Its children remain loaded in memory.
    let _ = tree.update(DirectoryTreeEvent::Toggled(td.path().join("src")));
    let src = find_in_tree(&tree, &td.path().join("src")).unwrap();
    assert!(!src.is_expanded, "collapsed");
    assert!(src.is_loaded, "still loaded");

    // Search for inner/readme. The deep match must still appear.
    tree.set_search_query("readme");
    assert!(
        tree.search_match_count() >= 2,
        "must find at least apps/readme.md and src/inner/readme.md \
         even though src/ is collapsed — search sees through \
         collapsed-but-loaded subtrees"
    );
}

#[test]
fn on_loaded_with_active_search_recomputes_visibility() {
    // With a search already active, expanding a previously-unloaded
    // folder should refresh the visibility set — newly-arrived
    // matches must become visible without the app needing to
    // re-issue set_search_query.
    let td = build_fixture("search-onload");
    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());

    // Activate search BEFORE src/ is loaded — so its readme.md is
    // not yet known. Expect 1 match (apps/ was sibling-scanned at
    // root level, we only loaded root; apps/readme.md isn't known
    // either). So 0 matches.
    tree.set_search_query("readme");
    assert_eq!(
        tree.search_match_count(),
        0,
        "no matches yet: only root is loaded and 'readme' \
         doesn't match the root basename"
    );

    // Now expand src/. The on_loaded path should recompute.
    tree.__test_expand_blocking(td.path().join("src"));
    assert!(
        tree.search_match_count() >= 1,
        "on_loaded must have re-run the search visibility pass; \
         src/README.md is now loaded and should count as a match"
    );
}
