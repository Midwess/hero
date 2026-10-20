---
feature: tcp
api_root: hero::tcp
targets: [native]
implies: []
catalog_schema: 1
---

# tcp

Bind dual-stack TCP and UDP listeners without tonic.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["tcp"]
}
```

## Availability

- API root: `hero::tcp`
- Target: Native
- Implied Hero features: None

## Enable When

- A native process uses Hero to bind or select a TCP/UDP port.

## Primary APIs

- `hero::tcp::listener::find_tcp_listener`
- `hero::tcp::listener::bind_tcp_listener`
- `hero::tcp::listener::find_udp_socket`

## Import Example

```rust,no_run
use hero::tcp::listener::find_tcp_listener;
```

## Environment

- `SERVICE_HOST`

## Selection Notes

- Hero's default feature set is empty.
- Enable `tcp` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
