---
feature: config
api_root: hero::config
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# config

Read the standard Midwess Kong and deployment configuration.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["config"]
}
```

## Availability

- API root: `hero::config`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- Code imports the shared Kong configuration.
- Code needs the configured public gateway URL.

## Primary APIs

- `hero::config::CONFIGS`
- `hero::config::Config`
- `hero::config::KongGatewayConfig`

## Import Example

```rust,no_run
use hero::config::CONFIGS;
```

## Environment

- `ENVIRONMENT`
- `KONG_GATEWAY_ADMIN_URL`
- `KONG_GATEWAY_URL`
- `KONG_GATEWAY_HOST`
- `KONG_GATEWAY_PORT`
- `PUBLIC_GATEWAY_HOST`
- `PUBLIC_GATEWAY_PORT`
- `PUBLIC_GATEWAY_WITH_SSL`

## Selection Notes

- Hero's default feature set is empty.
- Enable `config` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
