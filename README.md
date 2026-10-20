# Hero

Hero is the consolidated Midwess Rust library for cross-platform runtime
utilities and service integrations. It combines the retained parts of
`core-services` and `devlog-sdk` in one crate organized by capability.

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
    features = ["gateway", "grpc-server", "tcp"]
}
```

See [MIGRATION.md](MIGRATION.md) for import, feature, and API changes.

## Feature Presets

- `native-full`: all supported native integrations.
- `web-full`: browser HTTP/OPFS, retry, and distributed IDs.

Individual features are documented in `Cargo.toml`. Native and browser
dependencies are target-isolated, so `--all-features` compiles on both targets.

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
cargo test --features native-full
cargo clippy --all-targets --features native-full -- -D warnings
cargo check --target wasm32-unknown-unknown --features web-full
cargo clippy --target wasm32-unknown-unknown --features web-full -- -D warnings
scripts/check-exclusions.sh
scripts/check-features.sh
```

The repository preserves the complete histories of both source libraries.
Their import points are recorded in [HISTORY.md](HISTORY.md).

