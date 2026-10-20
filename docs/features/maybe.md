---
feature: maybe
api_root: hero::maybe
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# maybe

Express target-dependent sendability in generic APIs.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["maybe"]
}
```

## Availability

- API root: `hero::maybe`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- A generic API needs native Send/Sync bounds but relaxed WASM bounds.

## Primary APIs

- `hero::maybe::MaybeSend`
- `hero::maybe::MaybeSendSync`

## Import Example

```rust,no_run
use hero::maybe::MaybeSend;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `maybe` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
