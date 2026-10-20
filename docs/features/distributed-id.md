---
feature: distributed-id
api_root: hero::distributed_id
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# distributed-id

Generate Snowflake-style IDs without a coordination service.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "0.1",
    features = ["distributed-id"]
}
```

## Availability

- API root: `hero::distributed_id`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- One process generates IDs with a scoped worker ID.
- Code decodes Hero ID timestamps or generates scoped synthetic phone numbers.

## Primary APIs

- `hero::distributed_id::init_scoped_id_generator`
- `hero::distributed_id::try_gen_id`
- `hero::distributed_id::id_to_datetime`
- `hero::distributed_id::generate_phone_number`

## Import Example

```rust,no_run
use hero::distributed_id::init_scoped_id_generator;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `distributed-id` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
