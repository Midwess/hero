---
feature: distributed-id-etcd
api_root: hero::distributed_id
targets: [native]
implies: [distributed-id]
catalog_schema: 1
---

# distributed-id-etcd

Coordinate distributed-ID worker leases through etcd.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["distributed-id-etcd"]
}
```

## Availability

- API root: `hero::distributed_id`
- Target: Native
- Implied Hero features: `distributed-id`

## Enable When

- Multiple processes must generate IDs without worker-ID collisions.

## Primary APIs

- `hero::distributed_id::EtcdWorkerOptions`
- `hero::distributed_id::init_id_generator`
- `hero::distributed_id::shutdown_id_generator`
- `hero::distributed_id::etcd::WorkerLeaseGuard`

## Import Example

```rust,no_run
use hero::distributed_id::EtcdWorkerOptions;
```

## Environment

- `SNOWFLAKE_ETCD_ENDPOINTS`
- `SNOWFLAKE_ETCD_NAMESPACE`
- `SNOWFLAKE_ETCD_LEASE_TTL_SECS`

## Selection Notes

- Hero's default feature set is empty.
- Enable `distributed-id-etcd` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
