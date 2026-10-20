---
feature: throttle-future
api_root: hero::throttle_future
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# throttle-future

Coalesce frequent values before asynchronous processing.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "0.1",
    features = ["throttle-future"]
}
```

## Availability

- API root: `hero::throttle_future`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- Only the latest pending value should run after a delay.

## Primary APIs

- `hero::throttle_future::ThrottleFuture`
- `hero::throttle_future::ThrottleFn`

## Import Example

```rust,no_run
use hero::throttle_future::ThrottleFuture;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `throttle-future` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
