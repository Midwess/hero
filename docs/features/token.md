---
feature: token
api_root: hero::token
targets: [native]
implies: []
catalog_schema: 1
---

# token

Create and decode HS256 JSON Web Tokens.

## Enable

```toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["token"]
}
```

## Availability

- API root: `hero::token`
- Target: Native
- Implied Hero features: None

## Enable When

- A native authentication boundary issues or validates HS256 JWTs.

## Primary APIs

- `hero::token::jwt::create_jwt_token`
- `hero::token::jwt::decode_jwt_token`
- `hero::token::jwt::decode_jwt_token_without_validation`

## Import Example

```rust,no_run
use hero::token::jwt::create_jwt_token;
```

## Environment

This feature does not define environment variables.

## Selection Notes

- Hero's default feature set is empty.
- Enable `token` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
