# Hero MCP Resource Contract

An internal documentation MCP server can expose the versioned Hero bundle with
these resource shapes. The URI scheme is illustrative but stable within the
organization once adopted.

| Resource | MIME type | Contents |
| --- | --- | --- |
| `hero://<revision>/index` | `text/markdown` | Documentation index and architecture links. |
| `hero://<revision>/features` | `application/json` | `hero.features.json`. |
| `hero://<revision>/features/schema` | `application/schema+json` | Feature catalog JSON schema. |
| `hero://<revision>/features/<name>` | `text/markdown` | One focused feature page. |
| `hero://<revision>/manifest` | `text/plain` | The authoritative `Cargo.toml`. |
| `hero://<revision>/bundle-metadata` | `application/json` | Exact bundle version, revision, creation time, and distributable state. |
| `hero://<revision>/rustdoc/native/<path>` | `text/html` | Native rustdoc page. |
| `hero://<revision>/rustdoc/wasm/<path>` | `text/html` | WASM rustdoc page. |
| `hero://<revision>/migration` | `text/markdown` | Migration guide. |
| `hero://<revision>/llms` | `text/plain` | `llms.txt`. |
| `hero://<revision>/llms-full` | `text/plain` | Consolidated documentation corpus. |

## Required Metadata

Every response should include:

- crate name and Cargo version;
- full Git revision;
- target, when applicable;
- feature name and implied features, when applicable;
- source path within the bundle;
- bundle creation time.

The manifest, feature catalog, catalog schema, and bundle metadata resources
must be retrievable directly so clients can verify feature and revision claims.

## Query Guidance

The server should support listing feature resources and retrieving one resource
without requiring the complete corpus. Search results should preserve feature
and target metadata so callers can recommend minimal valid feature sets.
API-path lookup should follow the exact-item, longest-prefix, and ambiguity
rules in `docs/indexing.md`.

