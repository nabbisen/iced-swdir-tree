# Development

## Testing

```sh
cargo test --all-features
cargo clippy --all-targets --all-features
```

The crate ships 52 tests (27 unit + 24 integration + 1 doctest)
covering every filter mode, expand/collapse round-trips, selection
(including survival across runtime filter changes), stale-result
rejection, permission-denied, keyboard bindings, and custom
executor integration. See the [CHANGELOG](CHANGELOG.md) for the
per-release breakdown and [ROADMAP](ROADMAP.md) for what's next.
