# Hero Library Consolidation Plan

## Goal

Merge the retained parts of `core-services` and `devlog-sdk` into one Rust
library package named `hero`, rooted in this directory.

`core-services` is the structural base. The new crate will deliberately omit
these `core-services` areas:

- `db`
- `local_storage`
- `services`

They are not compatibility targets, will not be copied into `hero`, and will
not remain as deprecated modules. SDK code that currently depends on
`services::{Resolve, Errors}` will be replaced by module-local error and result
types instead.

## Target Structure

Build one crate, not a workspace of renamed copies:

```text
hero/
|- Cargo.toml              # package.name = "hero"
|- README.md
|- rustfmt.toml
|- src/
|  |- lib.rs
|  |- api_gateway/
|  |- config.rs
|  |- distributed_id/
|  |- env.rs
|  |- grpc_gateway/
|  |- logger/
|  |- retry/
|  |- s3/
|  |- shell/
|  |- smtp/
|  |- task/
|  |- tcp/
|  |- token/
|  |- utils/
|  `- wasm/
`- tests/
```

The structure follows `core-services`: its retained modules stay at their
current top-level paths, and the non-conflicting SDK modules are added beside
them. Do not introduce `core`, `sdk`, `core_services`, or `devlog_sdk` nesting
modules.

A retained API migration should therefore be mostly mechanical:

```rust
core_services::utils::cancellation::CancellationToken
// becomes
hero::utils::cancellation::CancellationToken

devlog_sdk::distributed_id::gen_id
// becomes
hero::distributed_id::gen_id
```

The removed service result paths have capability-specific replacements:

```rust
core_services::services::errors::Errors
// becomes one of
hero::api_gateway::Error
hero::tcp::Error
hero::s3::Error
hero::smtp::Error
```

The generic `Service<P, T>` trait and the database/local-storage APIs have no
replacement in Hero. There is intentionally no new global `Resolve`, `Result`,
or catch-all `Error` alias.

## Current-State Inventory

The two source repositories contain about 7,490 lines of Rust and 30 unit
tests. Excluding the three requested `core-services` areas removes about 1,561
lines, leaving roughly 5,929 lines as the initial merge scope.

Both repositories are currently versioned as `0.1.0`, have no release tags,
and have clean working trees at these source commits:

- `core-services`: `c1c62b6`, 203 commits
- `devlog-sdk`: `b125ecc`, 136 commits

### Included From `core-services`

| Area | Contents | Current feature/target |
| --- | --- | --- |
| Async/runtime utilities | Cancellation scopes, duplex streams, pool, throttling, yield container, time helpers | Mostly always exposed |
| Integrations | S3, SMTP/Crafter, JWT, shell process output | `s3`, `smtp`, `token`, `host_machine` |
| Platform support | Logger, browser HTTP, OPFS, JS/file extensions | `wasm`, `wasm-logger` plus target cfgs |
| Retry | Bounded retry macro | `retry` |
| Environment | Deployment environment parsing | Always exposed |

### Excluded From `core-services`

| Area | Excluded contents |
| --- | --- |
| `db` | Repository abstractions, redb, IndexedDB, IDs, tables, and database errors |
| `local_storage` | File/folder APIs, cursor abstraction, and ZIP streaming |
| `services` | `Service`, `Resolve`, `Errors`, response markers, and cloned results |

Because ZIP support is implemented inside `local_storage`, the old `zip` and
`zstd` features are also excluded from Hero.

### Included From `devlog-sdk`

| Area | Contents | Current feature/target |
| --- | --- | --- |
| API gateway | Kong service/route models and admin client | `gateway`, transitively `grpc-server` |
| Network | TCP, UDP, and tonic incoming-listener allocation | `tcp`, transitively `grpc-server` |
| gRPC client | Shared gateway `Channel` | `grpc-client` |
| Distributed IDs | Snowflake-style IDs, etcd worker leases, scoped phone numbers | Base generator always exposed; etcd under `distributed_id` |
| Runtime helpers | Infinite capped backoff and `AbortOnDrop` | Native targets |
| Configuration | Kong environment configuration and global `CONFIGS` | Always exposed |
| S3 construction | Environment-based S3 client setup currently held by `DevlogSdk` | Fold into `s3` |

### Direct Relationship

`devlog-sdk` already depends on `core-services`. After the exclusions, the
remaining direct coupling is:

- `services::Resolve` and `services::Errors` in TCP and Kong code;
- environment-based construction of `S3Client` in `DevlogSdk`;
- propagation of `grpc-server`, `smtp`, and `s3` features.

Replace the first dependency with errors owned by `api_gateway` and `tcp`
during the mechanical merge. Move S3 construction behavior into `crate::s3`
rather than retaining an SDK facade. There are no circular dependencies to
untangle.

## Findings That Must Shape the Merge

### 1. The checked-out crates are not standalone

`core-services` declares `schema = { path = "../schema/rust" }`. That path is
not present in this folder, and Cargo resolves path dependencies even when
they are optional. As checked out here, neither crate can be built until a
matching schema tree is supplied.

The dependency is used by retained code:

- S3 methods accept `schema::value::static_resource::{S3Path, StaticResource}`.
- SMTP accepts `schema::crafter::EmailTemplate` and calls the Crafter service.

`hero` must not retain an external relative path dependency.

### 2. Feature and target boundaries are inconsistent

The following checks fail in the current retained sources:

- `core-services --features host_machine`: missing Tokio `io-util`.
- `core-services --all-features` on native: browser modules are enabled on a
  native target and their target-only dependencies are unavailable.
- `core-services` default build for `wasm32-unknown-unknown`: logger and time
  code use JS/browser crates that are not enabled.
- `devlog-sdk` default build for `wasm32-unknown-unknown`: `rand/getrandom`
  lacks browser support.
- `devlog-sdk --features distributed_id` for WASM: etcd/Tokio networking is
  not a browser feature and pulls in unsupported `mio` code.

The native default, native backend, and current unit tests pass when the
missing schema path is restored in a temporary audit tree. The baseline is 17
passing `core-services` tests, one doc test, and 13 passing `devlog-sdk` tests.
None of those current tests live in the three excluded directories, so all 30
unit tests and the doc test remain applicable to Hero.

### 3. The dependency surface needs normalization

The merge should remove duplicate, excluded, or misleading declarations:

- Remove redb, IndexedDB, bincode, jwalk, rayon, async ZIP, and related
  serialization dependencies that were used only by excluded modules.
- Declare one AWS S3 SDK version instead of separate `1.41` and `1.76` ranges.
- Remove unused `aws-smithy-async`, `tonic-middleware`, SDK-side `lettre`, and
  SDK-side `futures-util` dependencies unless new code proves they are needed.
- Remove empty/no-op features such as `tokio` and `services`.
- Choose one TLS stack. SMTP currently enables both native TLS and rustls.
- Make Reqwest features explicit and consistent.
- Align tonic versions where schema compatibility allows it. The current full
  graph can contain tonic 0.11, 0.13, and 0.14.

Initial dependency policy for the merge:

- use rustls only for Reqwest and Lettre;
- use direct tonic `0.13.1` initially to remain compatible with current Hero
  consumers, while accepting etcd-client's private tonic `0.14` dependency;
- use Reqwest `0.12` with default features disabled and only the required
  `json` and `rustls-tls` features;
- use Lettre `0.11` with default features disabled and only builder, SMTP,
  Tokio, hostname, and rustls support;
- declare AWS S3 once, starting from the audited `1.142.x` line;
- support the latest stable Rust toolchain for Hero `0.1` without promising an
  MSRV until a lower toolchain is tested in CI;
- use stable-compatible rustfmt options only;
- distribute Hero by pinned Git revision until a license is selected, then
  decide whether crates.io publication is appropriate.

### 4. Some retained APIs need safety or reliability work

These should be isolated from the initial file move, but should block a stable
`1.0` release:

- `NeverSend<T>` has unconditional unsafe `Send` and `Sync` implementations.
  This is unsound if a wrapped value reaches another thread. Prefer removing
  artificial bounds on WASM or use a confinement wrapper with runtime checks.
- `gen_id`, `id_to_datetime`, S3 environment setup, and several builders panic
  on ordinary initialization/configuration errors. Add fallible APIs and keep
  panic-based functions only as temporary compatibility wrappers.
- `WorkerLeaseGuard::drop` spawns Tokio tasks and can fail when dropped outside
  an active runtime. Provide explicit async shutdown and make drop best-effort.
- `utils::pool::reponse` is misspelled and is widely consumed. Introduce the
  correct `response` module and keep a deprecated re-export at `reponse`.
- Kong operations repeatedly construct HTTP clients and often unwrap error
  response bodies. Share a configured client and preserve the original error
  if reading its body fails.

### 5. Excluding modules changes downstream migration scope

A local scan found 46 Cargo manifests and about 312 non-library Rust files
referencing one or both old crates. The excluded APIs alone account for:

- 234 `core_services::db` references across 80 files;
- 38 `core_services::local_storage` references across 18 files;
- 17 `core_services::services` references across 13 files.

Hero will not absorb those APIs. Consumers must handle them separately:

- replace database abstractions with application-owned or another database
  library;
- keep file/ZIP behavior in the application or move it to a dedicated storage
  crate;
- migrate retained `Resolve`/`Errors` uses to the error type owned by the
  relevant Hero module;
- do not migrate the generic `Service` trait into Hero.

This means the old `core-services` repository cannot be archived merely because
Hero is released. Archive it only after all consumers of the excluded modules
have moved elsewhere, or explicitly keep it as a frozen legacy dependency.

Several projects also contain copied forks. `midwess.ai/libs` matches these
source checkouts apart from `.DS_Store`, but `app-gateway`,
`bit-bridge`/`Bytover`, and `ai-workflow` differ. Only differences within the
retained Hero scope need to be reconciled; changes limited to excluded modules
remain outside this merge.

## Target Public API Strategy

### First Hero release

Keep retained module paths and change only the crate prefix wherever possible.

Special cases:

1. Replace the removed shared service error with module-owned errors:

   ```rust
   // Example inside hero::tcp
   pub type Result<T> = std::result::Result<T, Error>;
   ```

   `api_gateway`, `tcp`, `s3`, and `smtp` each expose their own error enum and
   local `Result<T>` alias. Shared source errors such as I/O or JSON are wrapped
   where they occur. Do not add a root catch-all error.

2. Merge both retry implementations into one module:

   ```text
   hero::retry!                        # bounded macro
   hero::retry::with_capped_backoff   # unbounded async helper
   ```

3. Keep cancellation only at `hero::utils::cancellation`. Do not add a second
   `hero::task` re-export.

4. Add `hero::utils::pool::response`, while temporarily re-exporting it from
   the legacy `hero::utils::pool::reponse` path.

5. Do not retain `sdk.rs`, `DevlogSdk`, or the `DependenciesInjection` trait.
   Move their only meaningful behavior to a typed
   `S3Config::from_env()?.build_client()` API under `hero::s3`.

6. Keep `hero::env::Environment` and `hero::config::CONFIGS` at their existing
   paths. Do not add convenience re-exports.

### Schema decoupling

Do not copy the schema project into Hero. Instead:

- add a small Hero-owned `ObjectPath { bucket, key }` type for S3 operations;
- make low-level S3 methods operate on `ObjectPath` and plain URLs;
- make SMTP send an already-rendered subject/body message;
- keep Crafter rendering and product-schema conversions outside Hero, in the
  consuming application or a separate product adapter crate.

Hero will not expose a `schema` or `midwess-schema` feature. The first release
is intentionally source-incompatible with the old schema-typed S3/SMTP APIs.

## Proposed Feature Model

Use kebab-case feature names and explicit `dep:` references. Keep underscore
aliases only when needed for migration.

| Feature | Enables | Target |
| --- | --- | --- |
| `default` | `distributed-id` plus platform-neutral utilities and config APIs | Native and WASM |
| `retry` | Retry macro on all targets and capped-backoff helper where supported | Native and WASM |
| `distributed-id` | Local Snowflake generator and scoped identifiers | Native and WASM |
| `shell` | Child-process stream support | Native |
| `tcp` | TCP/UDP listeners | Native |
| `grpc-client` | Gateway channel | Native |
| `grpc-server` | tonic status conversion and gRPC incoming listener support | Native |
| `gateway` | Kong models/client | Native |
| `distributed-id-etcd` | etcd worker allocation and lease maintenance | Native |
| `s3` | AWS S3 client | Native |
| `smtp` | SMTP transport | Native |
| `token` | JWT helpers | Native |
| `wasm` | Browser HTTP, OPFS, and JS extensions | WASM |
| `native-full` | `retry`, `distributed-id-etcd`, `shell`, `tcp`, `grpc-client`, `grpc-server`, `gateway`, `s3`, `smtp`, and `token` | Native |
| `web-full` | `retry`, `distributed-id`, and `wasm` | WASM |

The WASM target configuration must enable the JavaScript entropy backend used
by `rand/getrandom`, because scoped identifier generation remains part of the
cross-platform `distributed-id` feature.

Do not add legacy feature aliases. The old `host_machine`, `distributed_id`,
`backend`, `db-red`, `db-idb`, `local-storage`, `zip`, `zstd`, and `services`
features must not exist in Hero.

Target-only modules must have both feature and target cfgs. Enabling every
native feature must not compile browser modules, and `web-full` must not pull
in etcd, socket2, native Tokio networking, AWS, SMTP, or Kong. The current
`tcp::listener` file must gate tonic-specific `GrpcConnection` APIs with
`grpc-server`, so the standalone `tcp` feature does not require tonic.

## Implementation Phases

### Phase 0: Freeze and reconcile retained sources

1. Record the two source SHAs and stop editing copied libraries during the
   merge window.
2. Diff embedded copies against the two source repositories.
3. Review only differences in retained modules.
4. Classify each retained difference as upstreamable behavior,
   application-only customization, stale code, or generated/noise files.
5. Port valid retained fixes with provenance in commit messages.

Exit criterion: every meaningful fork difference in Hero's scope has an
explicit disposition.

### Phase 1: Create the Hero repository while preserving history

1. Initialize the root `hero` repository.
2. Import both Git histories under temporary prefixes using non-squashed
   subtrees or an equivalent history-preserving method.
3. Record origin repository URLs and source SHAs in the merge commit.
4. Add package metadata, README, license choice, MSRV/toolchain policy,
   stable-compatible rustfmt settings, and CI skeleton.

Exit criterion: both histories are reachable from Hero and the final tree has
no nested `.git` directories.

### Phase 2: Perform the scoped mechanical merge

1. Use retained `core-services/src` content as the initial root source tree.
2. Do not copy `src/db`, `src/local_storage`, or `src/services`.
3. Add the non-conflicting SDK modules at the same top-level paths.
4. Add module-local errors, then replace retained `services::Resolve` and
   `services::Errors` references with the owning module's `Result` and `Error`.
5. Move S3 environment construction out of `sdk.rs` and into `s3`; do not copy
   the SDK facade or dependency-injection trait.
6. Replace schema-typed S3/SMTP APIs with product-neutral APIs and remove the
   external schema path before attempting the first Hero build.
7. Merge the two retry directories and export both APIs.
8. Replace internal `core_services::...` references with `crate::...`.
9. Build a union `src/lib.rs` with clean target and feature gates.
10. Copy all unaffected tests unchanged and adapt only tests whose public API
    intentionally changed through schema decoupling.

Exit criterion: Hero passes the 30 existing unit tests and the current doc
test without containing the three excluded modules.

### Phase 3: Repair the manifest and platform model

1. Replace the two manifests with one normalized dependency list.
2. Remove dependencies and features used only by excluded modules.
3. Correct the `shell` Tokio I/O features and browser logger/time dependencies.
4. Move native-only dependencies into native target sections and browser-only
   dependencies into WASM target sections.
5. Add the exact `native-full` and `web-full` presets defined above.
6. Remove unused dependencies and converge AWS, tonic, Reqwest, and TLS
   versions/features.

Exit criterion: every supported preset resolves without files outside the
repository, and none of the excluded feature names exist.

### Phase 4: Harden retained APIs

1. Add the corrected `pool::response` path and deprecated typo alias.
2. Add fallible distributed-ID initialization/generation APIs.
3. Add explicit async shutdown for etcd lease guards.
4. Replace or contain `NeverSend` so thread-safety is not asserted without a
   real invariant.
5. Keep errors local to their capability and add cross-module conversions only
   when a public API genuinely composes those capabilities.
6. Replace eager environment panics with validated configuration constructors.
7. Reuse configured HTTP clients in gateway and SMTP integrations.

Exit criterion: the unconditional unsafe `Send`/`Sync` implementation is gone,
primary public APIs return errors instead of panicking on configuration or
initialization failure, and any deprecated panic-based wrappers are documented
and tested.

### Phase 5: Migrate consumers by capability

For each consuming repository:

1. Separate retained imports from excluded `db`, `local_storage`, and
   `services` imports.
2. Replace retained Cargo dependencies with `hero` and the required feature
   union.
3. Replace retained `core_services::` and `devlog_sdk::` prefixes with
   `hero::`.
4. Replace `DevlogSdk` usage with the new `hero::s3` constructor where it is
   actually needed; remove it entirely from consumers that only instantiate an
   empty facade.
5. Move excluded APIs to application-owned code or another dedicated crate.
6. Compile native and WASM targets separately where applicable.
7. Remove local copies only after both the Hero migration and excluded-module
   replacement are green.

Suggested order:

1. `ai-workflow` as a small gateway-only consumer.
2. `app-gateway`, including replacement of its database/service error imports.
3. `midwess.ai` for the broadest retained API coverage.
4. `bit-bridge`, then `Bytover`, because they have the heaviest database,
   local-storage, native, and browser coupling.
5. `cosin` and any Git-revision consumers.

Exit criterion: migrated projects use Hero only for retained capabilities and
have an explicit replacement for every excluded dependency.

### Phase 6: Release and legacy handling

1. Publish or pin the first Hero release.
2. Document retained import rewrites, feature mapping, and excluded APIs.
3. Mark `devlog-sdk` read-only once its consumers migrate.
4. Keep `core-services` frozen but available until consumers of `db`,
   `local_storage`, and `services` have migrated elsewhere.
5. Archive `core-services` only after those consumers are gone.

## Validation Matrix

The merge is complete only when these checks are automated:

| Check | Command intent |
| --- | --- |
| Native minimal | `cargo check` and `cargo test` |
| Native complete | check/test with `native-full` |
| Browser minimal | check default features for `wasm32-unknown-unknown` |
| Browser complete | check/test-compatible build with `web-full` |
| Individual features | each public feature checked independently where valid |
| Exclusion guard | fail CI if excluded directories, compatibility modules/symbols, or old feature names reappear |
| Formatting/lints | stable `cargo fmt --check`; Clippy for native and WASM presets |
| Docs | rustdoc for default, native-full, and web-full APIs |
| Packaging | package contains no external path dependency |
| Consumer smoke tests | one gateway server, one native app, and one WASM consumer |

Add missing retained behavior tests during the merge:

- gateway expression construction and Kong HTTP error handling;
- S3 and SMTP behavior behind mock transports;
- etcd lease allocation, reconnect, and shutdown;
- feature-gate compile tests;
- configuration parsing failures;
- non-panicking distributed-ID initialization errors;
- module-local error conversions for each retained integration.

## Acceptance Criteria

- The root has one Cargo package named `hero` and no nested active crates.
- `core-services` is the structural base of the source tree.
- Hero contains no `db`, `local_storage`, or `services` module.
- Hero contains no `sdk` module, `DevlogSdk`, or `DependenciesInjection`
  compatibility facade.
- Hero exposes no `db-red`, `db-idb`, `local-storage`, `zip`, `zstd`, or
  `services` feature, and no `host_machine`, `distributed_id`, or `backend`
  compatibility alias.
- Retained code uses module-local errors and does not add a compatibility
  `services` namespace or root catch-all error.
- Hero has no relative path dependency outside this repository.
- Native and WASM feature presets compile independently.
- The 30 existing unit tests and current doc test are preserved and passing.
- New tests cover retained feature/target failures found during this analysis.
- All direct dependencies are used and assigned to the correct target/feature.
- Valid retained changes from divergent embedded copies are not lost.
- The migration guide clearly lists excluded APIs and does not imply that Hero
  replaces them.

## Confirmed Decisions

1. The package and library name is `hero`.
2. Preserve both Git histories.
3. Use `core-services` as the clean structural base.
4. Merge SDK modules directly into that top-level structure.
5. Exclude `db`, `local_storage`, and `services` from Hero.
6. Remove the old SDK facade and place S3 construction under `hero::s3`.
7. Replace retained service error/result dependencies with module-local error
   APIs.
8. Keep schema outside Hero and expose product-neutral S3/SMTP APIs.
9. Support native and browser consumers with separate feature presets.
10. Provide no old module or feature compatibility layer; migration happens in
    consumers.

