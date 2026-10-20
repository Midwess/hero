---
feature: number
api_root: hero::number
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# number

Compute time-based exponential integer growth.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["number"]
}
```

## Availability

- API root: `hero::number`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- A bounded integer should grow exponentially with elapsed time.

## Primary APIs

- `hero::number::ExponentialGrowth`

## Import Example

```rust,no_run
use hero::number::ExponentialGrowth;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `number` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
