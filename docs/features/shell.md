---
feature: shell
api_root: hero::shell
targets: [native]
implies: []
catalog_schema: 1
---

# shell

Consume piped child stdout and stderr as one async stream.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["shell"]
}
```

## Availability

- API root: `hero::shell`
- Target: Native
- Implied Hero features: None

## Enable When

- A native process streams output from a spawned child.

## Primary APIs

- `hero::shell::StdStream`

## Import Example

```rust,no_run
use hero::shell::StdStream;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `shell` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
