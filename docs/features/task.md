---
feature: task
api_root: hero::task
targets: [native]
implies: []
catalog_schema: 1
---

# task

Abort an owned Tokio task when its guard drops.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["task"]
}
```

## Availability

- API root: `hero::task`
- Target: Native
- Implied Hero features: None

## Enable When

- A background Tokio task must not outlive its owner.

## Primary APIs

- `hero::task::AbortOnDrop`

## Import Example

```rust,no_run
use hero::task::AbortOnDrop;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `task` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
