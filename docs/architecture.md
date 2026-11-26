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

### Direct object uploads

The `s3` feature can issue exact-key presigned POST policies with storage-enforced
size bounds and expiration. Applications own resource rules, ownership checks,
byte validation, publication, and cleanup. See [direct POST uploads](s3-post.md)
for the browser contract and local storage verification.

### Browser HTTP

The `wasm` feature builds Fetch requests against the current global object,
including dedicated workers. Browser values stay on that worker. `send_streaming`
returns an owned `StreamingResponse`: status and headers precede consumption,
each `next_chunk` pulls one browser chunk, and dropping the response aborts the
request. `abort_handle` lets a worker-local cancellation observer stop an idle
response. Consumers attach their own cancellation/deadline policy to sending
and reading. The compatible `send` API returns the browser response with body
ownership transferred to its caller; `bytes` and `response` retain abort ownership
through complete body consumption.

Streaming uploads use the configured `stream_buffer_size`; the default is 1 MiB.
Request streaming requires browser support and a compatible server transport.
Consumers own upload admission, exact-length validation and buffered fallbacks.
`reject_redirects` can enforce a signed endpoint boundary without changing the
default browser redirect behavior for other Hero callers.

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

Configure scoped generators with `allow_ahead_of_time(milliseconds)` before
initialization. The default is zero. Positive values allow bursts to use future
sequence slots, so decoded timestamps can lead wall time by the configured
amount. If a scoped worker ID is reused, the next owner must start after the
last timestamp the previous owner could have issued. The etcd allocator
requires zero lead because it can reassign worker IDs without a timestamp
watermark.

### Owned asynchronous work

Compose `cancellation`, `task`, `retry`, `finally`, and `yield-container`
according to the ownership and shutdown behavior required by the component.

Use `child_timeout` for operation deadlines that end with their owner. Use
`cancel_after` when the timer must run independently of the scheduling handle.
In a tight loop, obtain the scoped token once and call its borrowed
`is_cancelled` method. Use `is_current_cancelled` when a scope lookup is needed
for each check and a missing scope should be treated as active work.


### Logging

The `logger` feature emits one logfmt record per line. The formatter uses
`ts`, `level`, `target`, and `msg` for the common record fields.
Applications can attach typed fields with `log` key-value records. Numeric
and boolean values remain unquoted, strings are escaped, and absent values
are omitted. Native applications can use `setup_with_context` for their own
fallback filter and task-local fields. Native stderr colors the severity on
supported terminals. Use `RUST_LOG_STYLE=never` for plain logfmt or `always`
to force colors. File tee output stays plain.
