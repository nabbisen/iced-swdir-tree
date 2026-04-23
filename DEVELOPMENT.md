# Development

## Testing

```sh
cargo test --all-features
cargo clippy --all-targets --all-features
```

The crate ships 100 tests (60 unit + 39 integration + 1 doctest)
covering every filter mode, expand/collapse round-trips, both
single- and multi-select (Replace / Toggle / ExtendRange, with
survival across runtime filter changes), stale-result rejection,
permission-denied, keyboard bindings (including Shift-range,
`Space`-toggle, and `Escape` to cancel drags), custom executor
integration, and the full drag-and-drop state machine
(is-valid-target rules, Pressed/Entered/Exited/Released/Cancelled
transitions, multi-item drag, descendant rejection). See the
[CHANGELOG](CHANGELOG.md) for the per-release breakdown and
[ROADMAP](ROADMAP.md) for what's next.
