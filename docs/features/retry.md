---
feature: retry
api_root: hero::retry
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# retry

Retry transient async operations with fixed or capped backoff.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["retry"]
}
```

## Availability

- API root: `hero::retry`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- An operation has explicitly classified transient failures.

## Primary APIs

- `hero::retry!`
- `hero::retry::with_capped_backoff`

## Import Example

```rust,no_run
use hero::retry;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `retry` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
