//! State machine for [`DirectoryTree::update`].
//!
//! The dispatcher lives here; each event variant is handled by a
//! dedicated submodule so that any one handler can grow independently
//! without the file crossing the "too long to read" threshold again:
//!
//! | Event                                   | Handler module           |
//! |-----------------------------------------|--------------------------|
//! | [`DirectoryTreeEvent::Toggled`]         | [`on_toggled`]           |
//! | [`DirectoryTreeEvent::Selected`]        | [`on_selected`]          |
//! | [`DirectoryTreeEvent::Drag`]            | [`on_drag`]              |
//! | [`DirectoryTreeEvent::Loaded`]          | [`on_loaded`]            |
//! | [`DirectoryTreeEvent::DragCompleted`]   | inline no-op             |
//!
//! `DragCompleted` is a broadcast event: the widget's state machine in
//! [`on_drag`] has already cleared its state by the time the message
//! reaches the dispatcher, so the dispatcher's job is just to route it
//! back through the app's message plumbing.
//!
//! The returned [`iced::Task`] carries any follow-up the widget needs
//! to emit — a `Loaded` for an in-flight scan, a delayed `Selected`
//! or `DragCompleted` for a completed drag gesture, etc.

use iced::Task;

use super::DirectoryTree;
use super::message::DirectoryTreeEvent;

mod on_drag;
mod on_loaded;
mod on_selected;
mod on_toggled;

impl DirectoryTree {
    /// Feed an event into the widget.
    ///
    /// Returns an `iced::Task` the parent should `.map(..)` back into
    /// its own message type. For `Selected` this is always
    /// [`Task::none()`]; for `Toggled` on an unloaded folder it carries
    /// the pending async scan; for `Loaded` it is again
    /// [`Task::none()`].
    ///
    /// Parent apps typically route every tree-related message here
    /// unconditionally:
    ///
    /// ```ignore
    /// fn update(&mut self, msg: MyMessage) -> Task<MyMessage> {
    ///     match msg {
    ///         MyMessage::Tree(e) => self.tree.update(e).map(MyMessage::Tree),
    ///     }
    /// }
    /// ```
    pub fn update(&mut self, msg: DirectoryTreeEvent) -> Task<DirectoryTreeEvent> {
        match msg {
            DirectoryTreeEvent::Toggled(path) => self.on_toggled(path),
            DirectoryTreeEvent::Selected(path, is_dir, mode) => {
                self.on_selected(path, is_dir, mode);
                Task::none()
            }
            DirectoryTreeEvent::Drag(msg) => self.on_drag(msg),
            DirectoryTreeEvent::DragCompleted { .. } => Task::none(),
            DirectoryTreeEvent::Loaded(payload) => {
                self.on_loaded(payload);
                Task::none()
            }
        }
    }
}

/// Compute the depth of `path` relative to `root`. Returns `0` if they
/// are equal, `1` for an immediate child, etc. If `path` does not
/// start with `root` (shouldn't happen in practice — every known node
/// descends from the root) we return `u32::MAX` so any depth limit
/// will trivially exclude it.
///
/// Shared between [`on_toggled`] (for the max-depth guard) and
/// [`DirectoryTree::__test_expand_blocking`] below. `pub(super)` so
/// submodules can import it via `use super::depth_of`.
pub(super) fn depth_of(root: &std::path::Path, path: &std::path::Path) -> u32 {
    let Ok(rel) = path.strip_prefix(root) else {
        return u32::MAX;
    };
    rel.components().count() as u32
}

impl DirectoryTree {
    /// Synchronously scan `path` and merge the result.
    ///
    /// **Test/helper API.** This duplicates the async `Toggled → scan →
    /// Loaded` round-trip but blocks on the scan, which is what
    /// integration tests need — iced's `Task` runtime machinery is
    /// private (see `iced_runtime::task::into_stream`), so driving a
    /// Task to completion from outside iced requires either standing
    /// up a window (overkill for unit-level tests) or bypassing the
    /// Task. This method does the latter.
    ///
    /// Real applications should not call this on the main thread —
    /// `scan_dir` blocks on `readdir` — and should route events
    /// through [`DirectoryTree::update`] instead, which delegates the
    /// scan to a worker thread.
    #[doc(hidden)]
    pub fn __test_expand_blocking(&mut self, path: std::path::PathBuf) {
        use super::message::LoadPayload;
        use std::sync::Arc;

        let depth = depth_of(&self.config.root_path, &path);
        // Skip the walker::scan Task entirely: call the blocking
        // primitive directly and hand-assemble the Loaded payload.
        let result = swdir::scan_dir(&path)
            .as_ref()
            .map(|e| super::walker::normalize_entries(e))
            .map_err(crate::Error::from);

        // Make sure generation matches — bump first, then attach.
        self.generation = self.generation.wrapping_add(1);
        let payload = LoadPayload {
            path: path.clone(),
            generation: self.generation,
            depth,
            result: Arc::new(result),
        };

        // Flip is_expanded, then feed the payload through the real
        // on_loaded so caching, error handling, etc. all go through
        // the production code path.
        if let Some(node) = self.root.find_mut(&path)
            && node.is_dir
        {
            node.is_expanded = true;
        }
        self.on_loaded(payload);
    }
}

#[cfg(test)]
mod tests;
