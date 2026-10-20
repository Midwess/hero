---
feature: api-gateway
api_root: hero::api_gateway
targets: [native]
implies: []
catalog_schema: 1
---

# api-gateway

Define and register Kong services, routes, and plugins.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["api-gateway"]
}
```

## Availability

- API root: `hero::api_gateway`
- Target: Native
- Implied Hero features: None

## Enable When

- The process constructs Kong route definitions.
- The process registers or removes services through the Kong Admin API.

## Primary APIs

- `hero::api_gateway::service::GatewayServiceBuilder`
- `hero::api_gateway::service::GatewayRouteBuilder`
- `hero::api_gateway::client::ApiGatewayClient`
- `hero::api_gateway::kong::client::KongGatewayAdminClient`

## Import Example

```rust,no_run
use hero::api_gateway::service::GatewayServiceBuilder;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `api-gateway` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
