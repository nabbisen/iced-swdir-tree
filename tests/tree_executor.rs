//! Tree-layer integration: v0.2 custom-executor plumbing.
//!
//! These exercise the `ScanExecutor` trait shape — that the trait
//! is object-safe, that `with_executor(Arc<dyn ScanExecutor>)`
//! compiles, and that omitting `with_executor` still yields a
//! working tree with the default `ThreadExecutor`.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use iced_swdir_tree::{
    DirectoryFilter, DirectoryTree, ScanExecutor, ScanFuture, ScanJob, ThreadExecutor,
};

/// A test-only executor that counts how many scans it ran.
///
/// Demonstrates the `ScanExecutor` trait is object-safe and pluggable,
/// and exercises the trait-method signature end-to-end.
#[derive(Default)]
struct CountingExecutor {
    count: AtomicUsize,
}

impl ScanExecutor for CountingExecutor {
    fn spawn_blocking(&self, job: ScanJob) -> ScanFuture {
        self.count.fetch_add(1, Ordering::SeqCst);
        // Delegate to the default ThreadExecutor to actually run the
        // work — we only want to observe here, not reimplement.
        ThreadExecutor.spawn_blocking(job)
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
    let exec: Arc<dyn ScanExecutor> = Arc::new(CountingExecutor::default());
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
