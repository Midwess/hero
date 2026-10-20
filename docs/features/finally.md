---
feature: finally
api_root: hero::finally
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# finally

Run collected asynchronous cleanup work when an owner drops.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "0.1",
    features = ["finally"]
}
```

## Availability

- API root: `hero::finally`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- An owner needs best-effort async cleanup on drop.

## Primary APIs

- `hero::finally::Finally`

## Import Example

```rust,no_run
use hero::finally::Finally;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `finally` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
