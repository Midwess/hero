---
feature: never-send
api_root: hero::never_send
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# never-send

Mark a value as intentionally local without faking auto-traits.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["never-send"]
}
```

## Availability

- API root: `hero::never_send`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- A wrapper should communicate thread-local ownership.

## Primary APIs

- `hero::never_send::NeverSend`

## Import Example

```rust,no_run
use hero::never_send::NeverSend;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `never-send` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
