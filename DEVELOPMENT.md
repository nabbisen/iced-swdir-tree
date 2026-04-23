# Development

## Testing

```sh
cargo test --all-features
cargo clippy --all-targets --all-features
```

The crate ships 25 tests covering every filter mode, expand/collapse
round-trips, selection, stale-result rejection, permission-denied, and
nonexistent paths. See the [CHANGELOG](CHANGELOG.md) for the per-release
breakdown and the roadmap.
