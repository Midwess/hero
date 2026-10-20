---
feature: cancellation
api_root: hero::cancellation
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# cancellation

Propagate structured, scoped cancellation through async work.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["cancellation"]
}
```

## Availability

- API root: `hero::cancellation`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- Async work needs parent-child cancellation.
- A future needs timeout or cancel-on-drop behavior.

## Primary APIs

- `hero::cancellation::CancellationToken`
- `hero::cancellation::FutureExtension`
- `hero::cancellation::DropGuard`

## Import Example

```rust,no_run
use hero::cancellation::CancellationToken;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `cancellation` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
