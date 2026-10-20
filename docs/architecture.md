# Hero Architecture

Hero provides reusable infrastructure and runtime patterns for the Midwess
organization. It sits between application code and external/runtime boundaries.

## Principles

### Explicit capabilities

Hero has an empty default feature set. A consumer's `Cargo.toml` records the
architectural capabilities it uses. A feature owns its dependencies and may
imply only features required by its implementation.

### Flat public modules

Public modules are available directly below `hero`. There is no generic
`utils` namespace. Feature names use kebab-case and module names use
snake_case.

### Domain independence

Hero accepts product-neutral values at infrastructure boundaries. Product
schemas, protobuf adapters, repositories, and business policy remain in the
consumer or a separate domain library.

### Local errors

Integrations own their error types. Hero does not define a catch-all service
error or generic application service abstraction.

### Target isolation

Native and WASM dependencies are isolated. Native-only features expose no
module on browser targets. Cross-platform features are validated independently
on both targets.

## Common Flows

### Service ingress

1. Bind a listener with `tcp` or `grpc-server` when Hero owns listener setup.
2. Build Kong routes with `api-gateway` when the process owns registration.
3. Add `config`, `logger`, and `cancellation` only when those modules are used.

### Service client

1. Enable `grpc-gateway`.
2. The feature enables `config` for gateway endpoint discovery.
3. Reuse `GrpcGatewayChannel` instead of rebuilding tonic channels per call.

### Distributed identity

1. Use `distributed-id` for one process or explicitly assigned worker IDs.
2. Use `distributed-id-etcd` when worker IDs require fleet coordination.
3. Explicitly shut down etcd-backed generation to revoke the lease.

### Owned asynchronous work

Compose `cancellation`, `task`, `retry`, `finally`, and `yield-container`
according to the ownership and shutdown behavior required by the component.

