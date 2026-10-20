---
feature: time
api_root: hero::time
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# time

Read epoch time in microseconds across targets.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["time"]
}
```

## Availability

- API root: `hero::time`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- Code needs one microsecond epoch-time API on native and browser targets.

## Primary APIs

- `hero::time::epoch_micro`

## Import Example

```rust,no_run
use hero::time::epoch_micro;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `time` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
