---
feature: smtp
api_root: hero::smtp
targets: [native]
implies: [cancellation]
catalog_schema: 1
---

# smtp

Send rendered HTML email with delivery cancellation.

## Enable

```toml
[dependencies]
hero = {
    package = "midwess-hero",
    version = "1",
    features = ["smtp"]
}
```

## Availability

- API root: `hero::smtp`
- Target: Native
- Implied Hero features: `cancellation`

## Enable When

- A native process sends already-rendered email through SMTP.

## Primary APIs

- `hero::smtp::client::SmtpClient`

## Import Example

```rust,no_run
use hero::smtp::client::SmtpClient;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `smtp` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
