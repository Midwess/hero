---
feature: stream
api_root: hero::stream
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# stream

Create in-memory duplex byte streams.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["stream"]
}
```

## Availability

- API root: `hero::stream`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- Two async endpoints need a bidirectional in-memory byte transport.

## Primary APIs

- `hero::stream::duplex`
- `hero::stream::duplex_with_config`
- `hero::stream::DuplexStream`

## Import Example

```rust,no_run
use hero::stream::duplex;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `stream` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
