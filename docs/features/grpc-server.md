---
feature: grpc-server
api_root: hero::tcp::listener
targets: [native]
implies: [tcp]
catalog_schema: 1
---

# grpc-server

Add tonic incoming-listener support to Hero's TCP pattern.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["grpc-server"]
}
```

## Availability

- API root: `hero::tcp::listener`
- Target: Native
- Implied Hero features: `tcp`

## Enable When

- A tonic server uses Hero to bind or select its listening port.

## Primary APIs

- `hero::tcp::listener::find_grpc_listener`
- `hero::tcp::listener::GrpcConnection`

## Import Example

```rust,no_run
use hero::tcp::listener::find_grpc_listener;
```

## Environment

- `SERVICE_HOST`

## Selection Notes

- Hero's default feature set is empty.
- Enable `grpc-server` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
