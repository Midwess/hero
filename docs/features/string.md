---
feature: string
api_root: hero::string
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# string

Use shared string validation helpers.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["string"]
}
```

## Availability

- API root: `hero::string`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- Code needs the shared email validation extension.

## Primary APIs

- `hero::string::StringExt`

## Import Example

```rust,no_run
use hero::string::StringExt;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `string` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
