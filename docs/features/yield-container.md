---
feature: yield-container
api_root: hero::yield_container
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# yield-container

Lend a value temporarily and return it on drop.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "0.1",
    features = ["yield-container"]
}
```

## Availability

- API root: `hero::yield_container`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- One owner temporarily lends a value that must be returned automatically.

## Primary APIs

- `hero::yield_container::YieldContainer`
- `hero::yield_container::Yieldable`
- `hero::yield_container::YieldError`

## Import Example

```rust,no_run
use hero::yield_container::YieldContainer;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `yield-container` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
