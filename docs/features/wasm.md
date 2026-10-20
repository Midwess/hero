---
feature: wasm
api_root: hero::wasm
targets: [wasm32-unknown-unknown]
implies: []
catalog_schema: 1
---

# wasm

Use browser HTTP, byte/file, and OPFS helpers.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["wasm"]
}
```

## Availability

- API root: `hero::wasm`
- Target: `wasm32-unknown-unknown`
- Implied Hero features: None

## Enable When

- A browser client needs Hero's Fetch/XHR, byte conversion, file metadata, or OPFS helpers.

## Primary APIs

- `hero::wasm::HttpClient`
- `hero::wasm::FetchRequest`
- `hero::wasm::XmlHttpRequest`
- `hero::wasm::extensions::VecExtension`
- `hero::wasm::opfs::SyncAccessHandleWriter`

## Import Example

```rust,no_run
use hero::wasm::HttpClient;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `wasm` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
