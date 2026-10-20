# Publishing Hero

## Crate Releases

Hero releases start at `1.0.0`. To release a new version, update the package
version in `Cargo.toml` and keep `hero.features.json` synchronized. A push to
`main` that increases the stable semantic version runs the release workflow.

The workflow validates native and WASM feature combinations, performs a Cargo
publish dry run, publishes the new version to crates.io, and creates the
matching `v<version>` GitHub release. Existing crates.io versions are not
republished. Publishing uses the organization-level `CARGO_REGISTRY_TOKEN`
secret.

The workflow can also be started manually to repair a missing crates.io or
GitHub release for the current version. It never uploads a version already
present in the crates.io index.

Do not reuse or decrease a released version. Changes to `Cargo.toml` that leave
the package version unchanged do not create a release.

docs.rs builds Hero with every feature enabled for the native default target.
The versioned documentation bundle additionally builds the complete supported
`wasm32-unknown-unknown` API with the target-compatible feature set. Public
items are guarded by `#![deny(missing_docs)]`, while CI treats Rustdoc warnings
(including broken intra-doc links) as errors. Because crates.io releases are
immutable, documentation corrections require a patch release.

## Documentation Bundles

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

- overview and architecture documents;
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
For the public Midwess Hero repository, the source documentation is public and
redistributable under the MIT license. Artifact visibility follows repository
and Actions permissions.

Distributable bundles require a clean Git worktree. For local preview only,
`ALLOW_DIRTY_DOCS=1 scripts/build-docs-bundle.sh` creates a bundle marked
`distributable: false`; do not upload or index that preview artifact.

An approved internal documentation site, Git provider connector, search
service, or MCP server can ingest a distributable artifact without rebuilding
documentation.

Before publishing a new bundle, confirm that it contains no secrets, internal
endpoints, customer data, or unpublished product information, and that the
revision retention policy is appropriate for the selected host.

`AGENTS.md` is included as repository operating guidance.
