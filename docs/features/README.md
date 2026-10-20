# Hero Feature Documentation

Hero has an empty default feature set. Select only the capabilities imported by
the consuming project.

| Feature | API root | Target | Purpose |
| --- | --- | --- | --- |
| [`api-gateway`](api-gateway.md) | `hero::api_gateway` | Native | Define and register Kong services, routes, and plugins. |
| [`cancellation`](cancellation.md) | `hero::cancellation` | Native and `wasm32-unknown-unknown` | Propagate structured, scoped cancellation through async work. |
| [`config`](config.md) | `hero::config` | Native and `wasm32-unknown-unknown` | Read the standard Midwess Kong and deployment configuration. |
| [`distributed-id`](distributed-id.md) | `hero::distributed_id` | Native and `wasm32-unknown-unknown` | Generate Snowflake-style IDs without a coordination service. |
| [`distributed-id-etcd`](distributed-id-etcd.md) | `hero::distributed_id` | Native | Coordinate distributed-ID worker leases through etcd. |
| [`env`](env.md) | `hero::env` | Native and `wasm32-unknown-unknown` | Normalize local, development, staging, and production checks. |
| [`finally`](finally.md) | `hero::finally` | Native and `wasm32-unknown-unknown` | Run collected asynchronous cleanup work when an owner drops. |
| [`grpc-gateway`](grpc-gateway.md) | `hero::grpc_gateway` | Native | Reuse a tonic channel through the configured gateway. |
| [`grpc-server`](grpc-server.md) | `hero::tcp::listener` | Native | Add tonic incoming-listener support to Hero's TCP pattern. |
| [`logger`](logger.md) | `hero::logger` | Native and `wasm32-unknown-unknown` | Apply Midwess logging defaults and throttled logging. |
| [`maybe`](maybe.md) | `hero::maybe` | Native and `wasm32-unknown-unknown` | Express target-dependent sendability in generic APIs. |
| [`never-send`](never-send.md) | `hero::never_send` | Native and `wasm32-unknown-unknown` | Mark a value as intentionally local without faking auto-traits. |
| [`number`](number.md) | `hero::number` | Native and `wasm32-unknown-unknown` | Compute time-based exponential integer growth. |
| [`pool`](pool.md) | `hero::pool` | Native and `wasm32-unknown-unknown` | Manage bounded reusable resources and waiter queues. |
| [`retry`](retry.md) | `hero::retry` | Native and `wasm32-unknown-unknown` | Retry transient async operations with fixed or capped backoff. |
| [`s3`](s3.md) | `hero::s3` | Native | Access S3-compatible object storage with product-neutral paths. |
| [`shell`](shell.md) | `hero::shell` | Native | Consume piped child stdout and stderr as one async stream. |
| [`smtp`](smtp.md) | `hero::smtp` | Native | Send rendered HTML email with delivery cancellation. |
| [`stream`](stream.md) | `hero::stream` | Native and `wasm32-unknown-unknown` | Create in-memory duplex byte streams. |
| [`string`](string.md) | `hero::string` | Native and `wasm32-unknown-unknown` | Use shared string validation helpers. |
| [`task`](task.md) | `hero::task` | Native | Abort an owned Tokio task when its guard drops. |
| [`tcp`](tcp.md) | `hero::tcp` | Native | Bind dual-stack TCP and UDP listeners without tonic. |
| [`throttle-future`](throttle-future.md) | `hero::throttle_future` | Native and `wasm32-unknown-unknown` | Coalesce frequent values before asynchronous processing. |
| [`time`](time.md) | `hero::time` | Native and `wasm32-unknown-unknown` | Read epoch time in microseconds across targets. |
| [`token`](token.md) | `hero::token` | Native | Create and decode HS256 JSON Web Tokens. |
| [`wasm`](wasm.md) | `hero::wasm` | `wasm32-unknown-unknown` | Use browser HTTP, byte/file, and OPFS helpers. |
| [`yield-container`](yield-container.md) | `hero::yield_container` | Native and `wasm32-unknown-unknown` | Lend a value temporarily and return it on drop. |

The machine-readable equivalent is [`hero.features.json`](../../hero.features.json).
The executable feature graph is [`Cargo.toml`](../../Cargo.toml).
