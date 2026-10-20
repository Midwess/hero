---
feature: grpc-gateway
api_root: hero::grpc_gateway
targets: [native]
implies: [config]
catalog_schema: 1
---

# grpc-gateway

Reuse a tonic channel through the configured gateway.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["grpc-gateway"]
}
```

## Availability

- API root: `hero::grpc_gateway`
- Target: Native
- Implied Hero features: `config`

## Enable When

- A native client calls gRPC services through Kong.

## Primary APIs

- `hero::grpc_gateway::channel::GrpcGatewayChannel`

## Import Example

```rust,no_run
use hero::grpc_gateway::channel::GrpcGatewayChannel;
```

## Environment

- `KONG_GATEWAY_URL`

## Selection Notes

- Hero's default feature set is empty.
- Enable `grpc-gateway` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
