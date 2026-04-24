# Custom scan executor

By default the widget spawns one `std::thread` per folder
expansion via `ThreadExecutor`. Apps that already run a
blocking-task pool (tokio, smol, rayon, ...) can route through
it by implementing `ScanExecutor`:

```rust,ignore
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use iced_swdir_tree::{ScanExecutor, ScanJob, ScanFuture, DirectoryTree};

struct TokioExecutor;

impl ScanExecutor for TokioExecutor {
    fn spawn_blocking(&self, job: ScanJob) -> ScanFuture {
        Box::pin(async move {
            tokio::task::spawn_blocking(job)
                .await
                .expect("scan task panicked")
        })
    }
}

let tree = DirectoryTree::new(root)
    .with_executor(Arc::new(TokioExecutor));
```

The default behaviour is unchanged if you don't call
`with_executor` — existing v0.1 code keeps working as-is.

## When to swap executors

The default `ThreadExecutor` spawns a fresh OS thread for every
folder expansion. That's correct but slightly wasteful for apps
that:

- **Already run a tokio/smol runtime** and want to reuse its
  blocking-task pool for better resource accounting.
- **Use `with_prefetch_limit(N)` at high `N`** — on
  `ThreadExecutor` that's `N` real threads per user expansion,
  which adds up. A bounded pool queues excess tasks instead. See
  [Parallel pre-expansion](prefetch.md).
- **Want to observe scan activity** for metrics, logging, or
  test instrumentation — a custom executor can wrap the default
  and tap into every `spawn_blocking` call.
