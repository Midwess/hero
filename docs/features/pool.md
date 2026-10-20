---
feature: pool
api_root: hero::pool
targets: [native, wasm32-unknown-unknown]
implies: [maybe]
catalog_schema: 1
---

# pool

Manage bounded reusable resources and waiter queues.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["pool"]
}
```

## Availability

- API root: `hero::pool`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: `maybe`

## Enable When

- Infrastructure resources are expensive to create and safe to reuse.

## Primary APIs

- `hero::pool::allocator::PoolResourceProvider`
- `hero::pool::allocator::PoolBuilder`
- `hero::pool::request::PoolRequestBuilder`
- `hero::pool::response::PoolResponse`

## Import Example

```rust,no_run
use hero::pool::allocator::PoolResourceProvider;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `pool` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
