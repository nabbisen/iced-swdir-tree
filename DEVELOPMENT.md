# Development

## Testing

```sh
cargo test --all-features
cargo clippy --all-targets --all-features
```

The crate ships 70 tests (41 unit + 28 integration + 1 doctest)
covering every filter mode, expand/collapse round-trips, both
single- and multi-select (Replace / Toggle / ExtendRange, with
survival across runtime filter changes), stale-result rejection,
permission-denied, keyboard bindings (including Shift-range and
`Space`-toggle), and custom executor integration. See the
[CHANGELOG](CHANGELOG.md) for the per-release breakdown and
[ROADMAP](ROADMAP.md) for what's next.
