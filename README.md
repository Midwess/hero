# Hero

Hero is the shared Rust library for the Midwess organization. It provides the
small, reusable patterns used by Midwess services, workers, native
applications, and browser/WASM clients at architectural boundaries.

Hero is intentionally not a framework. It does not own application domains,
databases, local storage, dependency-injection containers, or generic service
layers. It focuses on cancellation, resource pooling, identifiers, gateway
registration, network listeners, object storage, mail delivery, tokens,
logging, and browser integration.

Every capability is opt-in. The default feature set is empty, and a no-feature
build enables no normal dependency.

## Installation

Add Hero from crates.io and select only the features the project uses:

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["cancellation", "task"]
}
```

Or use Cargo:

```bash
cargo add midwess-hero --rename hero --features cancellation,task
```

Do not use `all-features` in an application dependency. Start with no features,
then add only the capabilities the application imports.

## Architectural Role

Keep business rules in the consuming application's domain and application
layers. Use Hero at infrastructure boundaries and in shared runtime code:

- propagate cancellation through nested asynchronous work;
- pool and automatically return reusable infrastructure resources;
- allocate IDs in one process or across an etcd-coordinated fleet;
- bind service listeners and register routes with the Midwess Kong gateway;
- connect tonic clients through the shared gRPC gateway;
- interact with S3-compatible storage and SMTP infrastructure;
- issue and validate JWTs at authentication boundaries;
- normalize logging, environments, retries, and task cleanup;
- expose browser HTTP, byte conversion, and OPFS helpers to WASM clients.

Product schemas, repositories, business entities, and product-specific
orchestration belong in the consuming application or a separate library.

## Common Combinations

The minimum column assumes the component uses every Hero capability named in
the scenario. Optional additions are conventions, not transitive requirements.

| Scenario | Minimum features | Optional additions |
| --- | --- | --- |
| Hero-managed TCP listener and Kong registration for an HTTP service | `api-gateway`, `tcp` | `config`, `logger`, `cancellation` when imported |
| Hero-managed gRPC listener and Kong registration | `api-gateway`, `grpc-server` | `config`, `logger`, `cancellation` when imported |
| Client calling services through Kong | `grpc-gateway` | `logger`, `cancellation` when imported |
| Worker using cancellation, owned tasks, and retry | `cancellation`, `task`, `retry` | `logger` |
| Fleet-wide ID producer | `distributed-id-etcd` | `logger` |
| Single-process ID producer | `distributed-id` | None |
| Backend using object storage | `s3` | `logger`, `retry` when imported |
| Backend sending rendered email | `smtp` | `logger`, `retry`, `task` when imported |
| Reusable infrastructure resource pool | `pool` | None |
| Browser HTTP/OPFS client | `wasm` | `cancellation`, `stream`, `time`, `logger` when imported |

If the application framework already binds its listener, omit
`tcp`/`grpc-server`. If route registration is handled by deployment tooling,
omit `api-gateway`. Add `config` only when importing `hero::config`.

## Feature Index

Feature names use kebab-case. Rust module names use snake_case. Select a feature
for details, exact API paths, environment variables, and related capabilities.

| Feature | API root | Target | Implies | Introduction |
| --- | --- | --- | --- | --- |
| [`api-gateway`](docs/features/api-gateway.md) | `hero::api_gateway` | Native | None | Define and register Kong services, routes, and plugins. |
| [`cancellation`](docs/features/cancellation.md) | `hero::cancellation` | Native/WASM | None | Propagate structured, scoped cancellation through async work. |
| [`config`](docs/features/config.md) | `hero::config` | Native/WASM | None | Read the standard Midwess Kong and environment configuration. |
| [`distributed-id`](docs/features/distributed-id.md) | `hero::distributed_id` | Native/WASM | None | Generate Snowflake-style IDs without a coordinator. |
| [`distributed-id-etcd`](docs/features/distributed-id-etcd.md) | `hero::distributed_id` | Native | `distributed-id` | Coordinate distributed-ID worker leases through etcd. |
| [`env`](docs/features/env.md) | `hero::env` | Native/WASM | None | Normalize local, development, staging, and production checks. |
| [`finally`](docs/features/finally.md) | `hero::finally` | Native/WASM | None | Run collected asynchronous cleanup work when an owner drops. |
| [`grpc-gateway`](docs/features/grpc-gateway.md) | `hero::grpc_gateway` | Native | `config` | Reuse a tonic channel through the configured gateway. |
| [`grpc-server`](docs/features/grpc-server.md) | `hero::tcp::listener` | Native | `tcp` | Add tonic incoming-listener support to Hero's TCP pattern. |
| [`logger`](docs/features/logger.md) | `hero::logger` | Native/WASM | None | Apply Midwess logging defaults and throttled logging. |
| [`maybe`](docs/features/maybe.md) | `hero::maybe` | Native/WASM | None | Express target-dependent sendability in generic APIs. |
| [`never-send`](docs/features/never-send.md) | `hero::never_send` | Native/WASM | None | Mark a value as intentionally local without faking auto-traits. |
| [`number`](docs/features/number.md) | `hero::number` | Native/WASM | None | Compute time-based exponential integer growth. |
| [`pool`](docs/features/pool.md) | `hero::pool` | Native/WASM | `maybe` | Manage bounded reusable resources and waiter queues. |
| [`retry`](docs/features/retry.md) | `hero::retry` | Native/WASM | None | Retry transient async operations with fixed or capped backoff. |
| [`s3`](docs/features/s3.md) | `hero::s3` | Native | None | Access S3-compatible object storage with product-neutral paths. |
| [`shell`](docs/features/shell.md) | `hero::shell` | Native | None | Consume piped child stdout and stderr as one async stream. |
| [`smtp`](docs/features/smtp.md) | `hero::smtp` | Native | `cancellation` | Send rendered HTML email with delivery cancellation. |
| [`stream`](docs/features/stream.md) | `hero::stream` | Native/WASM | None | Create in-memory duplex byte streams. |
| [`string`](docs/features/string.md) | `hero::string` | Native/WASM | None | Use shared string validation helpers. |
| [`task`](docs/features/task.md) | `hero::task` | Native | None | Abort an owned Tokio task when its guard drops. |
| [`tcp`](docs/features/tcp.md) | `hero::tcp` | Native | None | Bind dual-stack TCP/UDP listeners without tonic. |
| [`throttle-future`](docs/features/throttle-future.md) | `hero::throttle_future` | Native/WASM | None | Coalesce frequent values before asynchronous processing. |
| [`time`](docs/features/time.md) | `hero::time` | Native/WASM | None | Read epoch time in microseconds across targets. |
| [`token`](docs/features/token.md) | `hero::token` | Native | None | Create and decode HS256 JSON Web Tokens. |
| [`wasm`](docs/features/wasm.md) | `hero::wasm` | WASM | None | Use browser HTTP, byte/file, and OPFS helpers. |
| [`yield-container`](docs/features/yield-container.md) | `hero::yield_container` | Native/WASM | None | Lend a value temporarily and return it on drop. |

These entries are introductions, not bundled presets. List individual features
in the consuming `Cargo.toml` so architectural dependencies remain visible.

## Documentation

- [Feature documentation](docs/features/README.md)
- [Documentation publishing](docs/publishing.md)

## Development

```bash
cargo fmt --all -- --check
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
scripts/check-features.sh
scripts/check-exclusions.sh
scripts/check-docs.sh
scripts/build-docs-bundle.sh
```

The validation scripts keep Hero flat, opt-in, documented, and independently
buildable by feature on supported native and WASM targets.
