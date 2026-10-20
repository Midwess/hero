# Hero

Hero is the shared Rust library for the Midwess organization. It collects the
small, reusable design patterns used by Midwess services, workers, native
applications, and browser/WASM clients when they interact with the Midwess
architecture.

Hero is intentionally not a framework. It does not own an application's domain
model, database, local storage, dependency-injection container, or generic
service layer. It provides focused building blocks at architectural boundaries:
cancellation, resource pooling, identifiers, gateway registration, network
listeners, object storage, mail delivery, authentication tokens, logging, and
browser integration.

Every capability is opt-in. The default feature set is empty, and a no-feature
build enables no normal dependency. Consumers must select only the features
required by their architecture.

## Quick Start

Until a license and publication policy are selected, consume Hero by pinned Git
revision:

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["cancellation", "task"]
}
```

Do not use `--all-features` in an application dependency. Start with no
features, then add the exact capabilities the application imports.

```rust
use hero::cancellation::{CancellationToken, FutureExtension};

# async fn run() -> Result<(), hero::cancellation::TaskErrors> {
let cancellation = CancellationToken::timeout(std::time::Duration::from_secs(30));
perform_work().with_cancel(&cancellation).await?;
# Ok(())
# }
# async fn perform_work() {}
```

## Architectural Role

Midwess applications should keep business rules in their own domain and
application layers. Hero belongs at the edges and in shared runtime code.

Use Hero for these recurring patterns:

- propagate cancellation through nested asynchronous work;
- return pooled infrastructure resources automatically;
- allocate IDs consistently across one process or an etcd-coordinated fleet;
- bind service listeners and register routes with the Midwess Kong gateway;
- connect to services through the shared gRPC gateway;
- talk to S3-compatible object storage and SMTP infrastructure;
- issue and validate JWTs at authentication boundaries;
- normalize logging, environment detection, retries, and task cleanup;
- expose browser HTTP, byte conversion, and OPFS helpers to WASM clients.

Do not add product protobufs, repositories, business entities, or
application-specific orchestration to Hero. Product adapters should live in the
consuming application or in a separate domain-specific crate.

## Choosing Features

Use the smallest combination that matches the component being built.

### Common Midwess Combinations

The minimum column assumes the component uses every Hero capability named in
the scenario. Optional additions are common Midwess conventions, not
transitive requirements.

| Scenario | Minimum Hero features | Optional additions | What Hero provides |
| --- | --- | --- | --- |
| Hero-managed TCP listener and Kong registration for an HTTP service | `api-gateway`, `tcp` | `config`, `logger`, `cancellation` when those modules are imported | A TCP listener and Kong route registration. The application still supplies its HTTP framework/server. |
| Hero-managed gRPC listener and Kong registration | `api-gateway`, `grpc-server` | `config`, `logger`, `cancellation` when imported | A tonic incoming listener and Kong route registration; `grpc-server` already enables `tcp`. |
| Client calling services through Kong | `grpc-gateway` | `logger`, `cancellation` when imported | A reusable tonic channel; `grpc-gateway` already enables `config`. |
| Worker using scoped cancellation, owned tasks, and transient retry | `cancellation`, `task`, `retry` | `logger` | The three named lifecycle patterns; omit any feature for a pattern the worker does not use. |
| Fleet-wide ID producer | `distributed-id-etcd` | `logger` | etcd worker-ID coordination; `distributed-id-etcd` already enables `distributed-id`. |
| Single-process ID producer | `distributed-id` | None | Local Snowflake-style ID generation without networking. |
| Backend using object storage | `s3` | `logger`, `retry` when imported | S3-compatible object operations and validated storage configuration. |
| Backend sending rendered email | `smtp` | `logger`, `retry`, `task` when imported | HTML email delivery; `smtp` already enables `cancellation`. |
| Reusable infrastructure resource pool | `pool` | None | Bounded pooling and automatic resource return; `pool` already enables `maybe`. |
| Browser HTTP/OPFS client | `wasm` | `cancellation`, `stream`, `time`, `logger` only when imported | Browser HTTP, byte/file conversion, and OPFS helpers. |

These combinations are guidance, not presets. List the individual features in
the consuming `Cargo.toml` so architectural dependencies stay visible. If the
application framework already binds its listener, omit `tcp`/`grpc-server`. If
route registration is handled by another deployment component, omit
`api-gateway`. Add `config` only when the application imports `hero::config`.

## Feature Catalog

Feature names use kebab-case. Rust module names use snake_case. Target-specific
features expose no module on unsupported targets.

| Feature | Exact API root | Target | Implies | Enable when |
| --- | --- | --- | --- | --- |
| `api-gateway` | `hero::api_gateway` | Native | None | Building or registering Kong service/route definitions. |
| `cancellation` | `hero::cancellation` | Native/WASM | None | Work needs scoped, hierarchical, or timed cancellation. |
| `config` | `hero::config` | Native/WASM | None | Reading the standard Midwess Kong/environment configuration. |
| `distributed-id` | `hero::distributed_id` | Native/WASM | None | Generating IDs in one process or with an explicitly assigned worker ID. |
| `distributed-id-etcd` | `hero::distributed_id` | Native | `distributed-id` | Coordinating distributed-ID worker leases through etcd. |
| `env` | `hero::env` | Native/WASM | None | Normalizing local/dev/staging/production checks. |
| `finally` | `hero::finally` | Native/WASM | None | Running collected async cleanup work when an owner drops. |
| `grpc-gateway` | `hero::grpc_gateway` | Native | `config` | Connecting a tonic client through the configured gateway. |
| `grpc-server` | `hero::tcp::listener` | Native | `tcp` | Serving tonic through Hero's listener/port-selection pattern. |
| `logger` | `hero::logger` | Native/WASM | None | Using Midwess logging setup or throttled logging. |
| `maybe` | `hero::maybe` | Native/WASM | None | Writing generic APIs with target-dependent sendability bounds. |
| `never-send` | `hero::never_send` | Native/WASM | None | Marking a value as intentionally thread-local without faking auto-traits. |
| `number` | `hero::number` | Native/WASM | None | Using time-based exponential integer growth. |
| `pool` | `hero::pool` | Native/WASM | `maybe` | Managing reusable bounded resources. |
| `retry` | `hero::retry` and `hero::retry!` | Native/WASM | None | Retrying transient async operations. |
| `s3` | `hero::s3` | Native | None | Accessing S3-compatible object storage. |
| `shell` | `hero::shell` | Native | None | Merging piped child stdout/stderr into an async stream. |
| `smtp` | `hero::smtp` | Native | `cancellation` | Sending rendered HTML email with timeout/cancellation behavior. |
| `stream` | `hero::stream` | Native/WASM | None | Creating an in-memory duplex byte stream. |
| `string` | `hero::string` | Native/WASM | None | Using shared string validation helpers. |
| `task` | `hero::task` | Native | None | Aborting an owned Tokio task on drop. |
| `tcp` | `hero::tcp` | Native | None | Binding TCP/UDP listeners without tonic. |
| `throttle-future` | `hero::throttle_future` | Native/WASM | None | Coalescing frequent values before async processing. |
| `time` | `hero::time` | Native/WASM | None | Reading epoch time in microseconds. |
| `token` | `hero::token` | Native | None | Creating or decoding HS256 JWTs. |
| `wasm` | `hero::wasm` | WASM | None | Using browser HTTP, byte/file, or OPFS helpers. |
| `yield-container` | `hero::yield_container` | Native/WASM | None | Temporarily lending a value and returning it on drop. |

### Architecture And Integrations

#### `api-gateway` - `hero::api_gateway` - native only

Use when a service must describe or register HTTP/gRPC routes with the Midwess
Kong gateway.

- Main APIs: `hero::api_gateway::service::{GatewayServiceBuilder,
  GatewayRouteBuilder, GatewayRouteExpression}`,
  `hero::api_gateway::client::ApiGatewayClient`, and
  `hero::api_gateway::kong::client::KongGatewayAdminClient`.
- Patterns: declarative route construction, public/internal routing,
  gRPC-Web/CORS plugins, request-header transformation, idempotent registration.
- Enable with:

  ```toml
  hero = { ..., features = ["api-gateway"] }
  ```

#### `config` - `hero::config` - native and WASM

Use for the organization-standard Kong and deployment environment variables.

- Main APIs: `hero::config::{CONFIGS, Config, KongGatewayConfig}`.
- Reads `KONG_GATEWAY_ADMIN_URL`, `KONG_GATEWAY_URL`, `KONG_GATEWAY_HOST`,
  `KONG_GATEWAY_PORT`, `PUBLIC_GATEWAY_HOST`, `PUBLIC_GATEWAY_PORT`,
  `PUBLIC_GATEWAY_WITH_SSL`, and `ENVIRONMENT`.
- Automatically enabled by `grpc-gateway`.

#### `grpc-gateway` - `hero::grpc_gateway` - native only

Use when a client needs a lazily connected, reusable tonic channel through the
configured Midwess gateway.

- Main API: `hero::grpc_gateway::channel::GrpcGatewayChannel`.
- Automatically enables `config`.
- This is client-side gateway connectivity. It does not create a server.

#### `tcp` - `hero::tcp` - native only

Use to bind dual-stack TCP or UDP listeners, search for an available port, and
publish the configured `SERVICE_HOST`.

- Main APIs: `hero::tcp::listener::{find_tcp_listener, bind_tcp_listener,
  find_udp_socket, TcpConnection, UdpConnection}`.
- Does not pull tonic by itself.

#### `grpc-server` - extension of `hero::tcp` - native only

Use when a tonic server needs `TcpIncoming` plus Hero's port-selection pattern.

- Main APIs: `hero::tcp::listener::{find_grpc_listener, GrpcConnection}`.
- Automatically enables `tcp` and tonic transport.
- There is no separate `hero::grpc_server` module.

#### `s3` - `hero::s3` - native only

Use for S3-compatible object storage without coupling infrastructure code to
product protobufs.

- Main APIs: `hero::s3::{S3Config, S3Client, ObjectPath, MultipartUpload,
  UploadPart}`.
- Supports object CRUD, listing, presigned upload/download URLs, multipart
  upload completion, and stale multipart cleanup.
- `S3Config::from_env` reads `AWS_S3_REGION`, `AWS_ACCESS_KEY_ID`,
  `AWS_SECRET_ACCESS_KEY`, `AWS_ENDPOINT_URL`, and `AWS_S3_FORCE_PATH_STYLE`.
- Convert product-specific resource types to `ObjectPath` in the application.

#### `smtp` - `hero::smtp` - native only

Use to send already-rendered HTML email through SMTP.

- Main API: `hero::smtp::client::SmtpClient`; call the
  `SmtpClient::send_html` method.
- Automatically enables `cancellation` so sends have cancellation/timeout
  behavior.
- Template rendering and product protobufs belong outside Hero.

#### `token` - `hero::token` - native only

Use at authentication boundaries for HS256 JSON Web Tokens.

- Main APIs: `hero::token::jwt::{create_jwt_token, decode_jwt_token,
  decode_jwt_token_without_validation}`.
- The no-validation decoder is for inspection/debugging, not authorization.

#### `shell` - `hero::shell` - native only

Use when a child process has piped stdout and stderr that should be consumed as
one asynchronous stream.

- Main API: `hero::shell::StdStream`.
- Construct it with `TryFrom<tokio::process::Child>` after piping both streams.

#### `wasm` - `hero::wasm` - `wasm32-unknown-unknown` only

Use for browser-facing infrastructure.

- Main APIs: `hero::wasm::{HttpClient, FetchRequest, XmlHttpRequest, Body}`,
  `hero::wasm::extensions::{VecExtension, FileExtension}`, and
  `hero::wasm::opfs::SyncAccessHandleWriter`.
- Patterns: Fetch/XHR requests, streaming request bodies, response bytes,
  JavaScript byte conversion, file metadata, and OPFS synchronous access
  handles exposed through async I/O.
- Add other cross-platform features such as `cancellation`, `stream`, `time`,
  or `logger` separately when needed.

### Runtime And Control Flow

#### `cancellation` - `hero::cancellation` - native and WASM

Use for structured cancellation trees and task-local cancellation scopes.

- Main APIs: `hero::cancellation::{CancellationToken, FutureExtension,
  DropGuard, TaskErrors}`.
- Patterns: parent-to-child cancellation, scoped current tokens, cancel-on-drop,
  timeouts, and cancellation-aware futures.
- Cancellation is cooperative: work must observe the token.

#### `retry` - `hero::retry` - native and WASM

Use around transient operations.

- `hero::retry!` performs a bounded retry loop with a fixed delay.
- Native `hero::retry::with_capped_backoff` retries indefinitely with jittered
  exponential backoff capped at a maximum delay.
- The caller decides which errors are transient and when an outer cancellation
  boundary should stop an indefinite retry.

#### `task` - `hero::task` - native only

Use when a Tokio task must be aborted when its owner is dropped.

- Main API: `hero::task::AbortOnDrop<T>`.
- Pattern: RAII ownership for background tasks.

#### `finally` - `hero::finally` - native and WASM

Use to collect asynchronous cleanup futures and run them when the owner drops.

- Main API: `hero::finally::Finally`.
- Prefer explicit shutdown when completion must be observed; drop-triggered
  cleanup is best-effort lifecycle behavior.

#### `yield-container` - `hero::yield_container` - native and WASM

Use when one owner temporarily lends a value and needs it returned
automatically.

- Main APIs: `hero::yield_container::{YieldContainer, Yieldable, YieldError}`.
- Supports timed retrieval and return-on-drop.

#### `throttle-future` - `hero::throttle_future` - native and WASM

Use to coalesce frequent values and asynchronously process only the latest
pending value after a delay.

- Main APIs: `hero::throttle_future::{ThrottleFuture, ThrottleFn}`.
- Pattern: debounce/throttle expensive async updates.

### Identity

#### `distributed-id` - `hero::distributed_id` - native and WASM

Use for Midwess Snowflake-style 64-bit identifiers and scoped synthetic phone
numbers without a coordination service.

- Main APIs: `hero::distributed_id::{init_scoped_id_generator,
  init_scoped_id_generator_with_id, try_gen_id, try_gen_id_sync}` (sync is
  native only), plus `id_to_unix_timestamp`, `id_to_datetime`, and
  `generate_phone_number` from the same module.
- Prefer the fallible `try_*` APIs. Deprecated `gen_id` wrappers panic when the
  generator has not been initialized.
- Scoped worker IDs must be chosen so concurrently running producers do not
  collide.

#### `distributed-id-etcd` - extension of `hero::distributed_id` - native only

Use when multiple processes need coordinated worker IDs.

- Main APIs: `hero::distributed_id::{EtcdWorkerOptions, init_id_generator,
  shutdown_id_generator}` and `hero::distributed_id::etcd::WorkerLeaseGuard`.
- Automatically enables `distributed-id`.
- Reads `SNOWFLAKE_ETCD_ENDPOINTS`, `SNOWFLAKE_ETCD_NAMESPACE`, and
  `SNOWFLAKE_ETCD_LEASE_TTL_SECS` through `EtcdWorkerOptions::from_env`.
- Shut the generator down explicitly to revoke the etcd lease cleanly.

### Resource And Data Flow

#### `pool` - `hero::pool` - native and WASM

Use for bounded reusable infrastructure resources with idle cleanup and a
waiter queue.

- Main APIs: `hero::pool::allocator::{PoolResourceProvider, PoolBuilder,
  PoolAllocator}`, `hero::pool::request::PoolRequestBuilder`, and
  `hero::pool::response::PoolResponse`.
- Automatically enables `maybe`.
- `PoolResponse` returns its resource to the pool on drop.
- Use for generic resources only; Hero intentionally contains no database
  repository implementation.

#### `stream` - `hero::stream` - native and WASM

Use when two async endpoints need an in-memory duplex byte stream.

- Main APIs: `hero::stream::{duplex, duplex_with_config, DuplexStream,
  DuplexReader, DuplexWriter, DuplexConfig}`.
- Pattern: bidirectional adapters, protocol tests, and in-memory transport
  boundaries.

#### `never-send` - `hero::never_send` - native and WASM

Use as an explicit wrapper documenting that a value is local to its current
thread.

- Main API: `hero::never_send::NeverSend<T>`.
- It does not fake `Send` or `Sync`; the wrapped type's real auto-traits remain
  authoritative.

#### `maybe` - `hero::maybe` - native and WASM

Use in generic cross-platform APIs whose sendability differs by target.

- Main APIs: `hero::maybe::{MaybeSend, MaybeSendSync}`.
- On native targets they require thread-safety; on WASM they are marker traits.
- Usually enabled transitively through `pool` rather than selected directly.

### Environment, Logging, And Small Primitives

#### `logger` - `hero::logger` - native and WASM

Use for the Midwess logging defaults.

- Main APIs: `hero::logger::{setup, ThrottleLogger}`.
- Native: honors `RUST_LOG`; optional `FILE_LOG` and `FILE_LOG_PATH` tee logs
  to a file while retaining stderr output.
- WASM: writes to the browser console, installs the panic hook, and observes
  the global `window.__midwess_log` switch.

#### `env` - `hero::env` - native and WASM

Use for normalized deployment-environment checks.

- Main API: `hero::env::Environment` and its `current`, `is_local`,
  `is_dev_or_local`, and `as_str` methods.
- Reads `ENVIRONMENT`, then `DEPLOYMENT_ENV`; recognized values are local,
  development, staging, and production variants.

#### `time` - `hero::time` - native and WASM

Use when code needs epoch time in microseconds across native and browser
targets.

- Main API: `hero::time::epoch_micro`.

#### `number` - `hero::number` - native and WASM

Use for a time-based exponentially growing integer capped at a maximum.

- Main API: `hero::number::ExponentialGrowth`.

#### `string` - `hero::string` - native and WASM

Use for shared string validation helpers.

- Main API: `hero::string::StringExt::is_email`.

## Instructions For Coding Agents

When an agent adds Hero to a Midwess project, it should follow this procedure:

1. Read the project's target and architectural role before editing
   `Cargo.toml`.
2. Start with Hero's empty default feature set.
3. Match each imported `hero::<module>` to the feature catalog in this README.
4. Add only those features, plus documented transitive extensions.
5. Do not enable `all-features`, copy a broad feature list from another
   service, or add native-only features to a WASM target.
6. Keep domain models, repositories, product protobuf adapters, and business
   workflows outside Hero.
7. Run `cargo check` for the consumer's actual target after changing features.

Examples:

- `use hero::cancellation::CancellationToken` requires `cancellation`.
- `use hero::pool::allocator::PoolBuilder` requires `pool`; do not separately
  add `maybe` unless the project imports it directly.
- `use hero::tcp::listener::find_grpc_listener` requires `grpc-server`, not
  merely `tcp`.
- `use hero::distributed_id::init_id_generator` requires
  `distributed-id-etcd`; local generation needs only `distributed-id`.
- `use hero::grpc_gateway::channel::GrpcGatewayChannel` requires
  `grpc-gateway`, which enables `config`.

## What Hero Does Not Include

Hero deliberately excludes:

- database repositories and persistence abstractions;
- local filesystem/storage orchestration and ZIP packaging;
- a generic application service trait or catch-all error type;
- product schemas, protobuf-generated domain types, and Crafter templates;
- application dependency-injection containers;
- business workflows and product-specific policy.

See [MIGRATION.md](MIGRATION.md) when moving code from `core-services` or
`devlog-sdk`. See [HISTORY.md](HISTORY.md) for the imported repository history.

## Development

```bash
cargo fmt --all -- --check
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
scripts/check-features.sh
scripts/check-exclusions.sh
```

`scripts/check-features.sh` validates every feature independently on supported
native and WASM targets. `scripts/check-exclusions.sh` ensures Hero remains
flat, opt-in, and free from removed compatibility layers.

## AI And Agent Indexing

This README is the canonical human and agent feature-selection index for the
current Hero revision. `Cargo.toml` remains the executable source of truth; a
disagreement between it and this README is a documentation bug and
`Cargo.toml` wins. A future indexing design should build on this stable contract
rather than requiring an agent to infer features from source code.

Topics to decide next:

- whether to publish machine-readable feature metadata in a dedicated manifest
  such as `hero.features.json`;
- whether each feature should have a focused page with examples, target rules,
  environment variables, and related features;
- how to expose documentation to organization-approved AI providers and coding
  agents;
- how documentation versions map to pinned Git revisions or releases;
- what metadata and access controls are required before external indexing.

Until that design is agreed, agents should use this README, `Cargo.toml`, and
rustdoc as the authoritative sources in that order.
