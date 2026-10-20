# Migrating to Hero

Hero replaces the retained capabilities of `core-services` and `devlog-sdk`.
It is intentionally a breaking, fully opt-in migration with no default
features and no legacy module or feature aliases.

## Dependency

Replace both old dependencies with one Hero dependency and the union of the
retained features that the application uses:

```toml
# Before
core-services = { path = "../core-services", features = ["grpc-server"] }
devlog-sdk = { path = "../devlog-sdk", features = ["gateway", "tcp"] }

# After
hero = { path = "../hero", features = ["grpc-server", "api-gateway"] }
```

## Import Prefixes

Retained modules stay at the top level:

```text
core_services::utils::cancellation     -> hero::cancellation
core_services::utils::finally          -> hero::finally
core_services::utils::maybe            -> hero::maybe
core_services::utils::never_send       -> hero::never_send
core_services::utils::number           -> hero::number
core_services::utils::pool             -> hero::pool
core_services::utils::stream           -> hero::stream
core_services::utils::string           -> hero::string
core_services::utils::throttle_future  -> hero::throttle_future
core_services::utils::time             -> hero::time
core_services::utils::yield_container  -> hero::yield_container
core_services::logger         -> hero::logger
core_services::env            -> hero::env
core_services::s3             -> hero::s3
core_services::smtp           -> hero::smtp
core_services::token          -> hero::token
core_services::shell          -> hero::shell
core_services::wasm           -> hero::wasm

devlog_sdk::api_gateway       -> hero::api_gateway
devlog_sdk::config            -> hero::config
devlog_sdk::distributed_id    -> hero::distributed_id
devlog_sdk::grpc_gateway      -> hero::grpc_gateway
devlog_sdk::retry             -> hero::retry
devlog_sdk::task              -> hero::task
devlog_sdk::tcp               -> hero::tcp
```

## Removed Capabilities

Hero has no replacement for:

- `core_services::db`
- `core_services::local_storage`
- the generic `core_services::services::Service` abstraction

Move those capabilities into application-owned code or a dedicated library.
Retained Kong, TCP, S3, and SMTP APIs now expose errors in their own modules
instead of using `services::Errors`.

Hero also removes:

- `devlog_sdk::sdk::DevlogSdk`
- `devlog_sdk::sdk::DependenciesInjection`

Use `hero::s3::S3Config::from_env()?.build_client()` where an S3 client is
needed. Consumers that constructed an empty `DevlogSdk` can remove it.

## Feature Changes

| Old feature | Hero replacement |
| --- | --- |
| `host_machine` | `shell` |
| `distributed_id` | `distributed-id-etcd` |
| `gateway` | `api-gateway` |
| `grpc-client` | `grpc-gateway` |
| `backend` | Select every required capability individually |
| `db-red` | Removed |
| `db-idb` | Removed |
| `local-storage` | Removed |
| `zip` | Removed |
| `zstd` | Removed |
| `services` | Removed |

The previous default exports are now feature-gated. For example, using
`hero::cancellation` requires `features = ["cancellation"]`, and using the
pool requires `features = ["pool"]`. Cross-module requirements are encoded in
Cargo: `smtp` enables `cancellation`, `pool` enables `maybe`, `grpc-server`
enables `tcp`, and `distributed-id-etcd` enables `distributed-id`.

## API Changes

### Gateway builders

`GatewayRouteBuilder::build` and `GatewayServiceBuilder::build` now return
`Result` instead of panicking when required fields are missing.

### Distributed IDs

- Prefer `try_gen_id` and native `try_gen_id_sync`.
- `id_to_datetime` now returns `Result`.
- Scoped and etcd initialization report duplicate initialization.
- Call `shutdown_id_generator().await` before native etcd-backed shutdown.

The old `gen_id` functions remain deprecated panic-based wrappers for a short
source migration window.

### S3

- Schema protobuf types are replaced by `hero::s3::ObjectPath`.
- Environment parsing is handled by `S3Config::from_env`.
- Access to the underlying AWS client is read-only through `raw_client`.

Product-specific `StaticResource` conversions belong in the consuming
application or a separate adapter crate.

### SMTP

`SmtpClient::send_html` accepts a rendered subject and HTML body. Crafter and
product protobuf rendering are no longer library responsibilities.

### Pooling

- `pool::reponse` is removed; use `pool::response`.
- `PoolResourceProvider::new` is renamed to `create`.
- `PoolBuilder::build` and `PoolRequestBuilder::build` return `Result`.
- `NeverSend<T>` no longer unsafely implements `Send` or `Sync`.
- `NeverSend<T>` no longer provides implicit Serde forwarding; serialize the
  wrapped value explicitly when needed.

### Shell and gRPC

- Construct `StdStream` with `TryFrom<tokio::process::Child>`.
- `GrpcGatewayChannel::new` returns `Result`.
- `connect_with_timeout` returns the connected channel.

### Browser extensions

`VecExtension` methods are renamed to `to_uint8_array`, `to_js_value`, and
unsafe `view_uint8_array`. JSON bodies and missing URLs now return errors.

