# Design documents

These documents describe the **design** of `iced-swdir-tree` —
the decisions made, the invariants upheld, and the behavioural
contracts each feature must honour — independently of the iced
framework that hosts the reference implementation.

## Index

| Document | Audience |
| --- | --- |
| [Core design](core-design.md) | Anyone building a port; defines what the widget *is*, what it is *not*, and its non-negotiable invariants. |
| [Data model](data-model.md) | Implementors; every piece of in-memory state, its shape, and the guarantees it carries. |
| [State machine](state-machine.md) | Implementors; every valid state transition, the generation-counter protocol, and composability rules. |
| [Feature specifications](feature-specs.md) | Implementors and testers; precise behavioural spec for all ten features — the "test oracle" a port is written against. |

## Version

These documents describe **v0.7.2**.
