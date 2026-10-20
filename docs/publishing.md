# Publishing Hero Documentation

Hero documentation is built as a versioned artifact and is not automatically
published to a public site.

## Build

```bash
scripts/build-docs-bundle.sh
```

The script validates generated documentation, builds native and supported-WASM
rustdoc, records the Cargo version and Git revision, and creates a compressed
bundle under `target/docs-bundle/`.

## Bundle Contents

- overview, architecture, migration, and history documents;
- `Cargo.toml` and repository operating instructions;
- `llms.txt` and `llms-full.txt`;
- `hero.features.json` and its schema;
- one page per feature;
- native rustdoc;
- WASM rustdoc for the supported cross-platform feature set;
- `bundle-metadata.json` with the exact version, revision, creation time, and
  distributable state.

## Distribution

The CI workflow uploads the bundle as a repository-controlled build artifact.
Its visibility follows repository and Actions permissions; it is not private if
those permissions are public or broadly shared. The workflow refuses to upload
documentation from a public repository.

Distributable bundles require a clean Git worktree. For local preview only,
`ALLOW_DIRTY_DOCS=1 scripts/build-docs-bundle.sh` creates a bundle marked
`distributable: false`; do not upload or index that preview artifact.

An approved internal documentation site, Git provider connector, search
service, or MCP server can ingest a distributable artifact without rebuilding
documentation.

Before publishing outside the organization, confirm:

- repository visibility and source-code disclosure policy;
- selected license and documentation redistribution rights;
- provider data-retention and training controls;
- authentication and authorization for private documentation;
- revision retention and removal policy.

`AGENTS.md` is included as internal repository operating guidance. Treat it as
organization-internal material unless an explicit publication review approves
it for a broader audience.
