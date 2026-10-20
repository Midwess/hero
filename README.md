# Hero

Hero is the consolidated Midwess Rust library for cross-platform runtime
primitives and service integrations. Every capability is opt-in: the default
feature set is empty, so applications compile only the modules and third-party
dependencies they explicitly select.

## Scope

Hero includes:

- cancellation, streams, pooling, throttling, time, and other runtime helpers;
- local and etcd-coordinated distributed IDs;
- Kong API-gateway models and client;
- TCP, UDP, gRPC listener, and gRPC gateway helpers;
- S3, SMTP, JWT, shell, logging, and browser APIs.

Hero intentionally does not include the former `core-services` database,
local-storage, generic service abstraction, or the former `DevlogSdk` facade.

## Installation

Until a license and publication policy are selected, consume Hero by pinned
Git revision:

```toml
[dependencies]
hero = { git = "ssh://git@github.com/Midwess/hero.git", rev = "<commit>" }
```

Enable only the capabilities an application uses:

```toml
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["api-gateway", "grpc-server"]
}
```

See [MIGRATION.md](MIGRATION.md) for import, feature, and API changes.

## Features

Hero has no feature presets and no default capabilities. Enable modules
individually:

| Feature | Module |
| --- | --- |
| `api-gateway` | `hero::api_gateway` |
| `cancellation` | `hero::cancellation` |
| `config` | `hero::config` |
| `distributed-id` | `hero::distributed_id` |
| `distributed-id-etcd` | Adds etcd coordination to `distributed_id` |
| `env` | `hero::env` |
| `finally` | `hero::finally` |
| `grpc-gateway` | `hero::grpc_gateway` |
| `grpc-server` | Adds gRPC listener support to `hero::tcp` |
| `logger` | `hero::logger` |
| `maybe` | `hero::maybe` |
| `never-send` | `hero::never_send` |
| `number` | `hero::number` |
| `pool` | `hero::pool`; enables `maybe` |
| `retry` | `hero::retry` |
| `s3` | `hero::s3` |
| `shell` | `hero::shell` |
| `smtp` | `hero::smtp`; enables `cancellation` |
| `stream` | `hero::stream` |
| `string` | `hero::string` |
| `task` | `hero::task` |
| `tcp` | `hero::tcp` |
| `throttle-future` | `hero::throttle_future` |
| `time` | `hero::time` |
| `token` | `hero::token` |
| `wasm` | `hero::wasm` |
| `yield-container` | `hero::yield_container` |

`cargo build` with no features compiles no optional dependency. Native-only
features are not valid on WASM; `scripts/check-features.sh` validates every
feature on its supported targets.

## Examples

Create a Kong route:

```rust
use hero::api_gateway::service::{GatewayRouteBuilder, GatewayRouteExpression};

let route = GatewayRouteBuilder::new()
    .name("users")
    .path(GatewayRouteExpression::exact_or_subpath("/users"))
    .http()
    .build()?;
# Ok::<(), hero::api_gateway::service::BuildError>(())
```

Create an S3 client from validated environment configuration:

```rust,no_run
use hero::s3::S3Config;

let client = S3Config::from_env()?.build_client();
# Ok::<(), hero::s3::Error>(())
```

Generate distributed IDs without panicking:

```rust,no_run
use hero::distributed_id::{init_scoped_id_generator, try_gen_id};

init_scoped_id_generator("worker-1")?;
let id = try_gen_id().await?;
# Ok::<(), hero::distributed_id::generator::DistributedIdError>(())
```

## Development

```bash
cargo fmt --all -- --check
cargo test
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
scripts/check-exclusions.sh
scripts/check-features.sh
```

The repository preserves the complete histories of both source libraries.
Their import points are recorded in [HISTORY.md](HISTORY.md).

