//! Keyboard navigation adapter.
//!
//! [`DirectoryTree::handle_key`] translates a key press into an
//! appropriate [`DirectoryTreeEvent`], which the application routes
//! back through the widget's normal `update` flow. Applications are
//! expected to subscribe to `iced::keyboard::on_key_press` themselves
//! and pipe the key through this method — the widget stays focus-
//! neutral on purpose so apps can decide when the tree "has focus"
//! (e.g. only when it's visible, or only when a sidebar toggle is
//! on).
//!
//! # Bindings
//!
//! | Key | Behaviour |
//! |---|---|
//! | `Up` / `Down` | Move the selection to the previous / next visible row. |
//! | `Home` / `End` | Move the selection to the first / last visible row. |
//! | `Enter` | Toggle the currently-selected directory (no-op on files). |
//! | `Space` | Re-emit the current selection as a `Selected` event (idempotent). |
//! | `Left` | If the selection is an expanded directory → collapse it. Otherwise move the selection to its parent. |
//! | `Right` | If the selection is a collapsed directory → expand it. If it's an expanded directory with loaded children → move the selection to the first child. Otherwise no-op. |
//!
//! "Visible row" is defined the way the view draws the tree: the
//! root, plus every descendant whose every ancestor is expanded and
//! loaded. Filtered-out nodes are not visible, and therefore not
//! traversable with arrow keys.

use std::path::Path;

use iced::keyboard::{self, Modifiers, key::Named};

use super::DirectoryTree;
use super::message::DirectoryTreeEvent;
use super::node::TreeNode;

impl DirectoryTree {
    /// Translate a key press into the event that keyboard navigation
    /// should produce.
    ///
    /// Returns `None` when the key has no binding in the current
    /// state (e.g. `Right` on a file, or `Up` when no row is
    /// selected and the tree is empty). Callers can safely ignore
    /// the `None` case.
    ///
    /// This method is `&self` — it never mutates the tree. The
    /// returned event, if any, must be fed back through
    /// [`DirectoryTree::update`] like any other event so the
    /// existing state-machine (selection cursor, cache, generation
    /// counter) stays authoritative.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use iced::keyboard;
    /// // ...in your iced subscription function:
    /// fn subscription(app: &App) -> iced::Subscription<Message> {
    ///     keyboard::on_key_press(|key, mods| {
    ///         // Translate only when the tree has focus in your UI.
    ///         Some(Message::TreeKey(key, mods))
    ///     })
    /// }
    ///
    /// // ...in your update:
    /// Message::TreeKey(key, mods) => {
    ///     if let Some(event) = app.tree.handle_key(&key, mods) {
    ///         return app.tree.update(event).map(Message::Tree);
    ///     }
    ///     Task::none()
    /// }
    /// ```
    pub fn handle_key(
        &self,
        key: &keyboard::Key,
        _modifiers: Modifiers,
    ) -> Option<DirectoryTreeEvent> {
        // Only `Named` keys are bound at the moment — we don't handle
        // character keys (typing "a" to jump to entries starting with
        // "a" is a nice future feature, not a v0.2 one).
        let keyboard::Key::Named(named) = key else {
            return None;
        };

        // Precompute the flat list of visible rows — the same
        // ordering the view uses. Most bindings need to know
        // "where am I in this list" and "what's next / previous".
        let rows = visible_rows(&self.root);

        match named {
            Named::ArrowDown => self.move_selection(&rows, Direction::Next),
            Named::ArrowUp => self.move_selection(&rows, Direction::Prev),
            Named::Home => rows.first().map(select_event),
            Named::End => rows.last().map(select_event),
            Named::Enter => self.enter_action(),
            Named::Space => self.space_action(),
            Named::ArrowLeft => self.left_action(&rows),
            Named::ArrowRight => self.right_action(),
            _ => None,
        }
    }

    /// Return the event that moves selection along the flat visible-rows list.
    fn move_selection(
        &self,
        rows: &[VisibleRow<'_>],
        dir: Direction,
    ) -> Option<DirectoryTreeEvent> {
        if rows.is_empty() {
            return None;
        }
        // No selection yet → jump to the first (ArrowDown) or last
        // (ArrowUp) row. This matches the usual list-widget idiom.
        let Some(current) = self.selected_path.as_deref() else {
            return match dir {
                Direction::Next => rows.first().map(select_event),
                Direction::Prev => rows.last().map(select_event),
            };
        };
        // Find the current row index; if the selection is not
        // currently visible (filtered out / ancestor collapsed),
        // fall back to first/last as above.
        let Some(idx) = rows.iter().position(|r| r.node.path == current) else {
            return match dir {
                Direction::Next => rows.first().map(select_event),
                Direction::Prev => rows.last().map(select_event),
            };
        };
        let next_idx = match dir {
            Direction::Next => idx.saturating_add(1),
            Direction::Prev => idx.checked_sub(1)?,
        };
        rows.get(next_idx).map(select_event)
    }

    /// Enter → toggle the currently-selected directory; no-op on files.
    fn enter_action(&self) -> Option<DirectoryTreeEvent> {
        let path = self.selected_path.as_deref()?;
        let node = find(&self.root, path)?;
        if node.is_dir {
            Some(DirectoryTreeEvent::Toggled(path.to_path_buf()))
        } else {
            None
        }
    }

    /// Space → re-emit the current selection as a Selected event.
    /// Useful for "activate" without double-clicking.
    fn space_action(&self) -> Option<DirectoryTreeEvent> {
        let path = self.selected_path.as_deref()?;
        let node = find(&self.root, path)?;
        Some(DirectoryTreeEvent::Selected(
            path.to_path_buf(),
            node.is_dir,
        ))
    }

    /// Left:
    /// * expanded directory → collapse it
    /// * otherwise → move selection to parent (if visible)
    fn left_action(&self, rows: &[VisibleRow<'_>]) -> Option<DirectoryTreeEvent> {
        let path = self.selected_path.as_deref()?;
        let node = find(&self.root, path)?;
        if node.is_dir && node.is_expanded {
            return Some(DirectoryTreeEvent::Toggled(path.to_path_buf()));
        }
        // Find the row for the current path, then back up to the
        // nearest visible ancestor (depth-1 of the current depth).
        let current_idx = rows.iter().position(|r| r.node.path == path)?;
        let current_depth = rows[current_idx].depth;
        if current_depth == 0 {
            // Already at the root — nowhere further left to go.
            return None;
        }
        // Walk backwards until we find a row with strictly smaller
        // depth. That's the parent by construction, because the
        // flat row list is produced in pre-order traversal.
        let parent = rows[..current_idx]
            .iter()
            .rev()
            .find(|r| r.depth < current_depth)?;
        Some(select_event(parent))
    }

    /// Right:
    /// * collapsed directory → expand it
    /// * expanded directory with loaded children → move selection to first child
    /// * file → no-op
    fn right_action(&self) -> Option<DirectoryTreeEvent> {
        let path = self.selected_path.as_deref()?;
        let node = find(&self.root, path)?;
        if !node.is_dir {
            return None;
        }
        if !node.is_expanded {
            return Some(DirectoryTreeEvent::Toggled(path.to_path_buf()));
        }
        // Expanded directory: pick first visible child. We only
        // consider children that are actually in `node.children`
        // (already filtered by the current display filter).
        let first = node.children.first()?;
        Some(DirectoryTreeEvent::Selected(
            first.path.clone(),
            first.is_dir,
        ))
    }
}

// ----------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------

/// A single entry in the flat-rows list, paired with its depth.
///
/// Depth is cached here rather than recomputed via `strip_prefix`
/// because `visible_rows` builds the list anyway.
struct VisibleRow<'a> {
    node: &'a TreeNode,
    depth: u32,
}

#[derive(Clone, Copy)]
enum Direction {
    Next,
    Prev,
}

/// Return every visible row of the tree in the order the view
/// renders them.
///
/// Used by every navigation binding that needs to reason about
/// "previous" / "next" / "parent". The list is cheap to build —
/// each visible node costs one entry — and costs nothing when no
/// directories are expanded (it contains just the root).
fn visible_rows(root: &TreeNode) -> Vec<VisibleRow<'_>> {
    let mut out = Vec::new();
    collect_visible(root, 0, &mut out);
    out
}

fn collect_visible<'a>(node: &'a TreeNode, depth: u32, out: &mut Vec<VisibleRow<'a>>) {
    out.push(VisibleRow { node, depth });
    if node.is_dir && node.is_expanded && node.is_loaded {
        for child in &node.children {
            collect_visible(child, depth + 1, out);
        }
    }
}

/// O(depth) lookup by path. Mirrors `TreeNode::find_mut` but takes
/// a `&self` tree so we can use it from the `&self` key handler.
fn find<'a>(node: &'a TreeNode, target: &Path) -> Option<&'a TreeNode> {
    if node.path == target {
        return Some(node);
    }
    if !target.starts_with(&node.path) {
        return None;
    }
    for child in &node.children {
        if let Some(hit) = find(child, target) {
            return Some(hit);
        }
    }
    None
}

fn select_event(row: &VisibleRow<'_>) -> DirectoryTreeEvent {
    DirectoryTreeEvent::Selected(row.node.path.clone(), row.node.is_dir)
}

// ----------------------------------------------------------------
// Tests
// ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directory_tree::node::LoadedEntry;
    use std::path::PathBuf;

    /// Build a tiny synthetic tree: /r with children /r/a (dir,
    /// expanded, with children /r/a/aa, /r/a/ab) and /r/b (file).
    fn make_tree() -> DirectoryTree {
        let mut tree = DirectoryTree::new(PathBuf::from("/r"));
        tree.root.is_expanded = true;
        tree.root.is_loaded = true;
        let mut a = TreeNode::from_entry(&LoadedEntry {
            path: PathBuf::from("/r/a"),
            is_dir: true,
            is_symlink: false,
            is_hidden: false,
        });
        a.is_expanded = true;
        a.is_loaded = true;
        a.children.push(TreeNode::from_entry(&LoadedEntry {
            path: PathBuf::from("/r/a/aa"),
            is_dir: false,
            is_symlink: false,
            is_hidden: false,
        }));
        a.children.push(TreeNode::from_entry(&LoadedEntry {
            path: PathBuf::from("/r/a/ab"),
            is_dir: false,
            is_symlink: false,
            is_hidden: false,
        }));
        tree.root.children.push(a);
        tree.root.children.push(TreeNode::from_entry(&LoadedEntry {
            path: PathBuf::from("/r/b"),
            is_dir: false,
            is_symlink: false,
            is_hidden: false,
        }));
        tree
    }

    fn press(key: Named) -> iced::keyboard::Key {
        iced::keyboard::Key::Named(key)
    }

    #[test]
    fn arrow_down_from_no_selection_picks_first_row() {
        let tree = make_tree();
        let event = tree.handle_key(&press(Named::ArrowDown), Modifiers::default());
        match event {
            Some(DirectoryTreeEvent::Selected(p, _)) => assert_eq!(p, PathBuf::from("/r")),
            other => panic!("expected Selected, got {other:?}"),
        }
    }

    #[test]
    fn arrow_down_moves_forward_in_visible_order() {
        let mut tree = make_tree();
        tree.selected_path = Some(PathBuf::from("/r"));
        tree.sync_selection_flag(&PathBuf::from("/r"));
        let e = tree.handle_key(&press(Named::ArrowDown), Modifiers::default());
        match e {
            Some(DirectoryTreeEvent::Selected(p, _)) => assert_eq!(p, PathBuf::from("/r/a")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn arrow_up_moves_backward() {
        let mut tree = make_tree();
        tree.selected_path = Some(PathBuf::from("/r/a/aa"));
        tree.sync_selection_flag(&PathBuf::from("/r/a/aa"));
        let e = tree.handle_key(&press(Named::ArrowUp), Modifiers::default());
        match e {
            Some(DirectoryTreeEvent::Selected(p, _)) => assert_eq!(p, PathBuf::from("/r/a")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn arrow_up_at_top_returns_none() {
        let mut tree = make_tree();
        tree.selected_path = Some(PathBuf::from("/r"));
        tree.sync_selection_flag(&PathBuf::from("/r"));
        assert!(
            tree.handle_key(&press(Named::ArrowUp), Modifiers::default())
                .is_none()
        );
    }

    #[test]
    fn enter_on_folder_toggles() {
        let mut tree = make_tree();
        tree.selected_path = Some(PathBuf::from("/r/a"));
        tree.sync_selection_flag(&PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::Enter), Modifiers::default()) {
            Some(DirectoryTreeEvent::Toggled(p)) => assert_eq!(p, PathBuf::from("/r/a")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn enter_on_file_is_noop() {
        let mut tree = make_tree();
        tree.selected_path = Some(PathBuf::from("/r/b"));
        tree.sync_selection_flag(&PathBuf::from("/r/b"));
        assert!(
            tree.handle_key(&press(Named::Enter), Modifiers::default())
                .is_none()
        );
    }

    #[test]
    fn left_on_expanded_folder_collapses() {
        let mut tree = make_tree();
        tree.selected_path = Some(PathBuf::from("/r/a"));
        tree.sync_selection_flag(&PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::ArrowLeft), Modifiers::default()) {
            Some(DirectoryTreeEvent::Toggled(p)) => assert_eq!(p, PathBuf::from("/r/a")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn left_on_file_moves_to_parent() {
        let mut tree = make_tree();
        tree.selected_path = Some(PathBuf::from("/r/a/aa"));
        tree.sync_selection_flag(&PathBuf::from("/r/a/aa"));
        match tree.handle_key(&press(Named::ArrowLeft), Modifiers::default()) {
            Some(DirectoryTreeEvent::Selected(p, _)) => assert_eq!(p, PathBuf::from("/r/a")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn right_on_collapsed_folder_expands() {
        let mut tree = make_tree();
        // Collapse /r/a so Right expands it.
        tree.root.children[0].is_expanded = false;
        tree.selected_path = Some(PathBuf::from("/r/a"));
        tree.sync_selection_flag(&PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::ArrowRight), Modifiers::default()) {
            Some(DirectoryTreeEvent::Toggled(p)) => assert_eq!(p, PathBuf::from("/r/a")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn right_on_expanded_folder_moves_to_first_child() {
        let mut tree = make_tree();
        tree.selected_path = Some(PathBuf::from("/r/a"));
        tree.sync_selection_flag(&PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::ArrowRight), Modifiers::default()) {
            Some(DirectoryTreeEvent::Selected(p, _)) => assert_eq!(p, PathBuf::from("/r/a/aa")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn home_end_jump_to_boundaries() {
        let mut tree = make_tree();
        tree.selected_path = Some(PathBuf::from("/r/a"));
        tree.sync_selection_flag(&PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::Home), Modifiers::default()) {
            Some(DirectoryTreeEvent::Selected(p, _)) => assert_eq!(p, PathBuf::from("/r")),
            other => panic!("{other:?}"),
        }
        match tree.handle_key(&press(Named::End), Modifiers::default()) {
            Some(DirectoryTreeEvent::Selected(p, _)) => assert_eq!(p, PathBuf::from("/r/b")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn space_re_emits_selection() {
        let mut tree = make_tree();
        tree.selected_path = Some(PathBuf::from("/r/b"));
        tree.sync_selection_flag(&PathBuf::from("/r/b"));
        match tree.handle_key(&press(Named::Space), Modifiers::default()) {
            Some(DirectoryTreeEvent::Selected(p, is_dir)) => {
                assert_eq!(p, PathBuf::from("/r/b"));
                assert!(!is_dir);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unbound_keys_return_none() {
        let tree = make_tree();
        assert!(
            tree.handle_key(&press(Named::Escape), Modifiers::default())
                .is_none()
        );
        assert!(
            tree.handle_key(
                &iced::keyboard::Key::Character("x".into()),
                Modifiers::default()
            )
            .is_none()
        );
    }
}
