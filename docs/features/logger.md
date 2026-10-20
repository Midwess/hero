---
feature: logger
api_root: hero::logger
targets: [native, wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# logger

Apply Midwess logging defaults and throttled logging.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["logger"]
}
```

## Availability

- API root: `hero::logger`
- Target: Native and `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- A process uses the shared logging setup.
- Repeated log messages should be aggregated.

## Primary APIs

- `hero::logger::setup`
- `hero::logger::ThrottleLogger`

## Import Example

```rust,no_run
use hero::logger::setup;
```

## Environment

- `RUST_LOG`
- `FILE_LOG`
- `FILE_LOG_PATH`
- `SANDBOX_ID`

## Selection Notes

- Hero's default feature set is empty.
- Enable `logger` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
