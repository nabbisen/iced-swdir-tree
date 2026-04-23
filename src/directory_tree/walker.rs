//! Asynchronous directory traversal layer.
//!
//! [`scan`] produces an [`iced::Task`] that hands back one
//! directory's worth of normalized children. We intentionally delegate
//! the blocking I/O to a worker thread via `std::thread::spawn` instead
//! of running inside an async runtime: `swdir::scan_dir` is
//! synchronous by design, and spinning up a thread per user-initiated
//! expansion is far cheaper than dragging in `tokio` or `async-std`.
//! This keeps the widget runtime-agnostic — iced's default smol-based
//! executor, a tokio-driven iced app, or anything else works equally.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use swdir::{DirEntry, ScanError, scan_dir};

#[cfg(test)]
use super::config::DirectoryFilter;
use super::message::{DirectoryTreeEvent, LoadPayload};
use super::node::LoadedEntry;
use crate::Error;

/// Scan `path` asynchronously and produce the corresponding
/// [`DirectoryTreeEvent::Loaded`] event.
///
/// `generation` and `depth` are threaded through to the resulting
/// payload so the update layer can both (a) discard stale results
/// when the user has re-collapsed/re-expanded the folder in the
/// meantime, and (b) place the new children under the right node
/// without a full tree walk.
///
/// The filter is **not** applied here — the result is a
/// raw-but-normalized listing, and the update layer runs the current
/// filter over it before populating `TreeNode::children`. This lets
/// [`DirectoryTree::set_filter`](crate::DirectoryTree) re-derive
/// filtered children from the cache without re-scanning.
pub(crate) fn scan(path: PathBuf, generation: u64, depth: u32) -> Task<DirectoryTreeEvent> {
    // Clone the target path for the message-builder closure. We can't
    // reuse the one moved into the worker thread, and we need it in
    // both branches (Ok and Err) of the result.
    let target = path.clone();
    Task::perform(
        async move {
            // `thread::spawn(..).join()` keeps swdir synchronous but
            // moves it off the iced executor thread. If the worker
            // panics (rare — effectively OOM-only), surface it as an
            // Io error rather than letting it propagate and tear
            // the iced runtime down.
            let raw = std::thread::spawn(move || scan_dir(&path))
                .join()
                .unwrap_or_else(|_| {
                    Err(ScanError::Io {
                        path: PathBuf::new(),
                        source: std::io::Error::other("scan worker thread panicked"),
                    })
                });
            raw.as_ref()
                .map(|entries| normalize_entries(entries))
                .map_err(Error::from)
        },
        move |result| {
            DirectoryTreeEvent::Loaded(LoadPayload {
                path: target.clone(),
                generation,
                depth,
                result: Arc::new(result),
            })
        },
    )
}

/// Normalize raw swdir entries into the crate's own [`LoadedEntry`]
/// representation.
///
/// Intentionally does *not* apply [`DirectoryFilter`]; that happens
/// in the update layer so cached entries can be re-filtered on the
/// fly when the display filter changes. We do sort here (directories
/// first, then files, each group alphabetically) because sorting is
/// a property of the listing — not of the filter — and is free to
/// do once while the whole slice is in hand.
pub(crate) fn normalize_entries(entries: &[DirEntry]) -> Vec<LoadedEntry> {
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        let path = e.path().to_path_buf();
        let is_dir = e.is_dir();
        let is_symlink = e.is_symlink();
        let is_hidden = is_hidden(&path, e);
        out.push(LoadedEntry {
            path,
            is_dir,
            is_symlink,
            is_hidden,
        });
    }
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
            .cmp(
                &b.path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase(),
            ),
    });
    out
}

/// Platform-aware hidden-file detection.
///
/// * **Unix**: file name begins with `.` (dotfile convention).
/// * **Windows**: the `HIDDEN` attribute bit is set on the file's
///   metadata. Also falls back to the dotfile heuristic so that files
///   that traveled from Unix (e.g. synced `.git` folders) still get
///   detected.
/// * **Elsewhere**: dotfile heuristic only.
#[cfg(unix)]
fn is_hidden(_path: &Path, entry: &DirEntry) -> bool {
    entry
        .path()
        .file_name()
        .map(|n| {
            let s = n.to_string_lossy();
            s.starts_with('.') && s.as_ref() != "." && s.as_ref() != ".."
        })
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_hidden(_path: &Path, entry: &DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    // FILE_ATTRIBUTE_HIDDEN = 0x2
    const HIDDEN_ATTR: u32 = 0x2;
    let hidden_bit = entry
        .metadata()
        .map(|m| m.file_attributes() & HIDDEN_ATTR != 0)
        .unwrap_or(false);
    let dotfile = entry
        .path()
        .file_name()
        .map(|n| n.to_string_lossy().starts_with('.'))
        .unwrap_or(false);
    hidden_bit || dotfile
}

#[cfg(not(any(unix, windows)))]
fn is_hidden(_path: &Path, entry: &DirEntry) -> bool {
    entry
        .path()
        .file_name()
        .map(|n| n.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
}

// -------------------------------------------------------------------
// Unit tests
// -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Minimal self-cleaning temp directory.
    struct TmpDir(PathBuf);

    impl TmpDir {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let p = std::env::temp_dir().join(format!(
                "iced-swdir-tree-test-{}-{}-{}",
                std::process::id(),
                nanos,
                tag
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).expect("create tmpdir");
            Self(p)
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn filter_folders_only_drops_files() {
        let td = TmpDir::new("folders-only");
        fs::create_dir(td.0.join("sub")).unwrap();
        fs::write(td.0.join("file.txt"), b"").unwrap();
        let raw = scan_dir(&td.0).unwrap();
        let out: Vec<_> = normalize_entries(&raw)
            .into_iter()
            .filter(|e| e.passes(DirectoryFilter::FoldersOnly))
            .collect();
        assert_eq!(out.len(), 1);
        assert!(out[0].is_dir);
    }

    #[test]
    fn filter_files_and_folders_drops_hidden() {
        let td = TmpDir::new("no-hidden");
        fs::write(td.0.join(".secret"), b"").unwrap();
        fs::write(td.0.join("visible.txt"), b"").unwrap();
        let raw = scan_dir(&td.0).unwrap();
        let out: Vec<_> = normalize_entries(&raw)
            .into_iter()
            .filter(|e| e.passes(DirectoryFilter::FilesAndFolders))
            .collect();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path.file_name().unwrap(), "visible.txt");
    }

    #[test]
    fn filter_all_includes_hidden() {
        let td = TmpDir::new("with-hidden");
        fs::write(td.0.join(".secret"), b"").unwrap();
        fs::write(td.0.join("visible.txt"), b"").unwrap();
        let raw = scan_dir(&td.0).unwrap();
        let out: Vec<_> = normalize_entries(&raw)
            .into_iter()
            .filter(|e| e.passes(DirectoryFilter::AllIncludingHidden))
            .collect();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn folders_sort_before_files() {
        let td = TmpDir::new("sort");
        fs::create_dir(td.0.join("zebra")).unwrap();
        fs::write(td.0.join("alpha"), b"").unwrap();
        let raw = scan_dir(&td.0).unwrap();
        let out = normalize_entries(&raw);
        assert_eq!(out.len(), 2);
        assert!(out[0].is_dir);
        assert!(!out[1].is_dir);
    }
}
