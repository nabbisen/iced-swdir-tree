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
//! | `Shift + Up` / `Shift + Down` | Extend the selected range toward the previous / next visible row. |
//! | `Home` / `End` | Move the selection to the first / last visible row. |
//! | `Shift + Home` / `Shift + End` | Extend the selected range to the first / last visible row. |
//! | `Enter` | Toggle the currently-selected directory (no-op on files). |
//! | `Space` / `Ctrl + Space` | Toggle the currently-active path in or out of the selected set. |
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
use super::node::{TreeNode, VisibleRow};
use super::selection::SelectionMode;

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
    /// existing state-machine (selection set, cache, generation
    /// counter) stays authoritative.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use iced::keyboard;
    /// // ...in your iced subscription function:
    /// fn subscription(app: &App) -> iced::Subscription<Message> {
    ///     keyboard::listen().map(|event| match event {
    ///         keyboard::Event::KeyPressed { key, modifiers, .. } =>
    ///             Message::TreeKey(key, modifiers),
    ///         _ => Message::Noop,
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
        modifiers: Modifiers,
    ) -> Option<DirectoryTreeEvent> {
        // Only `Named` keys are bound at the moment — we don't handle
        // character keys (typing "a" to jump to entries starting with
        // "a" is a nice future feature, not a v0.3 one).
        let keyboard::Key::Named(named) = key else {
            return None;
        };

        // Navigation mode: Shift extends, everything else replaces.
        let nav_mode = if modifiers.shift() {
            SelectionMode::ExtendRange
        } else {
            SelectionMode::Replace
        };

        // Precompute the flat list of visible rows — the same
        // ordering the view uses. Most bindings need to know
        // "where am I in this list" and "what's next / previous".
        let rows = self.root.visible_rows();

        match named {
            Named::ArrowDown => self.move_selection(&rows, Direction::Next, nav_mode),
            Named::ArrowUp => self.move_selection(&rows, Direction::Prev, nav_mode),
            Named::Home => rows.first().map(|r| select_event(r, nav_mode)),
            Named::End => rows.last().map(|r| select_event(r, nav_mode)),
            Named::Enter => self.enter_action(),
            // Space and Ctrl+Space both toggle the active path in
            // and out of the selected set — the standard
            // tree-widget Space behaviour. This is a deliberate
            // change from v0.2, where Space re-emitted the current
            // selection as Replace.
            Named::Space => self.toggle_active(),
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
        mode: SelectionMode,
    ) -> Option<DirectoryTreeEvent> {
        if rows.is_empty() {
            return None;
        }
        // No active path yet → jump to the first (ArrowDown) or
        // last (ArrowUp) row. This matches the usual list-widget
        // idiom. The mode is carried through so Shift+arrow from a
        // fresh tree still produces an ExtendRange event (which
        // will fall back to Replace in update() given there's no
        // anchor yet).
        let Some(current) = self.active_path.as_deref() else {
            return match dir {
                Direction::Next => rows.first().map(|r| select_event(r, mode)),
                Direction::Prev => rows.last().map(|r| select_event(r, mode)),
            };
        };
        let Some(idx) = rows.iter().position(|r| r.node.path == current) else {
            return match dir {
                Direction::Next => rows.first().map(|r| select_event(r, mode)),
                Direction::Prev => rows.last().map(|r| select_event(r, mode)),
            };
        };
        let next_idx = match dir {
            Direction::Next => idx.saturating_add(1),
            Direction::Prev => idx.checked_sub(1)?,
        };
        rows.get(next_idx).map(|r| select_event(r, mode))
    }

    /// Enter → toggle the currently-active directory; no-op on files.
    fn enter_action(&self) -> Option<DirectoryTreeEvent> {
        let path = self.active_path.as_deref()?;
        let node = find(&self.root, path)?;
        if node.is_dir {
            Some(DirectoryTreeEvent::Toggled(path.to_path_buf()))
        } else {
            None
        }
    }

    /// Space → toggle the active path in/out of the selected set.
    fn toggle_active(&self) -> Option<DirectoryTreeEvent> {
        let path = self.active_path.as_deref()?;
        let node = find(&self.root, path)?;
        Some(DirectoryTreeEvent::Selected(
            path.to_path_buf(),
            node.is_dir,
            SelectionMode::Toggle,
        ))
    }

    /// Left:
    /// * expanded directory → collapse it
    /// * otherwise → move selection to parent (if visible)
    fn left_action(&self, rows: &[VisibleRow<'_>]) -> Option<DirectoryTreeEvent> {
        let path = self.active_path.as_deref()?;
        let node = find(&self.root, path)?;
        if node.is_dir && node.is_expanded {
            return Some(DirectoryTreeEvent::Toggled(path.to_path_buf()));
        }
        let current_idx = rows.iter().position(|r| r.node.path == path)?;
        let current_depth = rows[current_idx].depth;
        if current_depth == 0 {
            return None;
        }
        let parent = rows[..current_idx]
            .iter()
            .rev()
            .find(|r| r.depth < current_depth)?;
        Some(select_event(parent, SelectionMode::Replace))
    }

    /// Right:
    /// * collapsed directory → expand it
    /// * expanded directory with loaded children → move selection to first child
    /// * file → no-op
    fn right_action(&self) -> Option<DirectoryTreeEvent> {
        let path = self.active_path.as_deref()?;
        let node = find(&self.root, path)?;
        if !node.is_dir {
            return None;
        }
        if !node.is_expanded {
            return Some(DirectoryTreeEvent::Toggled(path.to_path_buf()));
        }
        let first = node.children.first()?;
        Some(DirectoryTreeEvent::Selected(
            first.path.clone(),
            first.is_dir,
            SelectionMode::Replace,
        ))
    }
}

// ----------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------

#[derive(Clone, Copy)]
enum Direction {
    Next,
    Prev,
}

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

fn select_event(row: &VisibleRow<'_>, mode: SelectionMode) -> DirectoryTreeEvent {
    DirectoryTreeEvent::Selected(row.node.path.clone(), row.node.is_dir, mode)
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

    /// Shorthand: set the tree's active + anchor + selected_paths to
    /// `path` as if a Replace-click had just happened. Bypasses the
    /// update() machinery for test brevity.
    fn put_cursor_at(tree: &mut DirectoryTree, path: PathBuf) {
        tree.active_path = Some(path.clone());
        tree.anchor_path = Some(path.clone());
        tree.selected_paths = vec![path.clone()];
        tree.sync_selection_flags();
    }

    #[test]
    fn arrow_down_from_no_selection_picks_first_row() {
        let tree = make_tree();
        let event = tree.handle_key(&press(Named::ArrowDown), Modifiers::default());
        match event {
            Some(DirectoryTreeEvent::Selected(p, _, mode)) => {
                assert_eq!(p, PathBuf::from("/r"));
                assert_eq!(mode, SelectionMode::Replace);
            }
            other => panic!("expected Selected, got {other:?}"),
        }
    }

    #[test]
    fn arrow_down_moves_forward_in_visible_order() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r"));
        let e = tree.handle_key(&press(Named::ArrowDown), Modifiers::default());
        match e {
            Some(DirectoryTreeEvent::Selected(p, _, mode)) => {
                assert_eq!(p, PathBuf::from("/r/a"));
                assert_eq!(mode, SelectionMode::Replace);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn shift_arrow_down_emits_extend_range() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r"));
        let e = tree.handle_key(&press(Named::ArrowDown), Modifiers::SHIFT);
        match e {
            Some(DirectoryTreeEvent::Selected(p, _, mode)) => {
                assert_eq!(p, PathBuf::from("/r/a"));
                assert_eq!(mode, SelectionMode::ExtendRange);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn arrow_up_moves_backward() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r/a/aa"));
        let e = tree.handle_key(&press(Named::ArrowUp), Modifiers::default());
        match e {
            Some(DirectoryTreeEvent::Selected(p, _, _)) => assert_eq!(p, PathBuf::from("/r/a")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn arrow_up_at_top_returns_none() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r"));
        assert!(
            tree.handle_key(&press(Named::ArrowUp), Modifiers::default())
                .is_none()
        );
    }

    #[test]
    fn enter_on_folder_toggles() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::Enter), Modifiers::default()) {
            Some(DirectoryTreeEvent::Toggled(p)) => assert_eq!(p, PathBuf::from("/r/a")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn enter_on_file_is_noop() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r/b"));
        assert!(
            tree.handle_key(&press(Named::Enter), Modifiers::default())
                .is_none()
        );
    }

    #[test]
    fn left_on_expanded_folder_collapses() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::ArrowLeft), Modifiers::default()) {
            Some(DirectoryTreeEvent::Toggled(p)) => assert_eq!(p, PathBuf::from("/r/a")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn left_on_file_moves_to_parent() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r/a/aa"));
        match tree.handle_key(&press(Named::ArrowLeft), Modifiers::default()) {
            Some(DirectoryTreeEvent::Selected(p, _, _)) => {
                assert_eq!(p, PathBuf::from("/r/a"))
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn right_on_collapsed_folder_expands() {
        let mut tree = make_tree();
        tree.root.children[0].is_expanded = false;
        put_cursor_at(&mut tree, PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::ArrowRight), Modifiers::default()) {
            Some(DirectoryTreeEvent::Toggled(p)) => assert_eq!(p, PathBuf::from("/r/a")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn right_on_expanded_folder_moves_to_first_child() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::ArrowRight), Modifiers::default()) {
            Some(DirectoryTreeEvent::Selected(p, _, _)) => {
                assert_eq!(p, PathBuf::from("/r/a/aa"))
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn home_end_jump_to_boundaries() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::Home), Modifiers::default()) {
            Some(DirectoryTreeEvent::Selected(p, _, _)) => assert_eq!(p, PathBuf::from("/r")),
            other => panic!("{other:?}"),
        }
        match tree.handle_key(&press(Named::End), Modifiers::default()) {
            Some(DirectoryTreeEvent::Selected(p, _, _)) => assert_eq!(p, PathBuf::from("/r/b")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn shift_home_end_emits_extend_range() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r/a"));
        match tree.handle_key(&press(Named::Home), Modifiers::SHIFT) {
            Some(DirectoryTreeEvent::Selected(p, _, mode)) => {
                assert_eq!(p, PathBuf::from("/r"));
                assert_eq!(mode, SelectionMode::ExtendRange);
            }
            other => panic!("{other:?}"),
        }
        match tree.handle_key(&press(Named::End), Modifiers::SHIFT) {
            Some(DirectoryTreeEvent::Selected(p, _, mode)) => {
                assert_eq!(p, PathBuf::from("/r/b"));
                assert_eq!(mode, SelectionMode::ExtendRange);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn space_toggles_active_path() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r/b"));
        match tree.handle_key(&press(Named::Space), Modifiers::default()) {
            Some(DirectoryTreeEvent::Selected(p, is_dir, mode)) => {
                assert_eq!(p, PathBuf::from("/r/b"));
                assert!(!is_dir);
                assert_eq!(mode, SelectionMode::Toggle);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ctrl_space_also_toggles() {
        let mut tree = make_tree();
        put_cursor_at(&mut tree, PathBuf::from("/r/b"));
        match tree.handle_key(&press(Named::Space), Modifiers::CTRL) {
            Some(DirectoryTreeEvent::Selected(_, _, mode)) => {
                assert_eq!(mode, SelectionMode::Toggle);
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
