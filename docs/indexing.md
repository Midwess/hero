# Documentation Indexing Guidance

This document defines how Hero documentation should be ingested by approved
search, retrieval, coding, and documentation systems.

## Canonical Inputs

Use these sources in priority order:

1. `Cargo.toml` for the executable feature graph and dependency truth.
2. `hero.features.json` for machine-readable selection metadata.
3. `docs/features/<feature>.md` for focused feature guidance.
4. rustdoc for item-level signatures and type documentation.
5. `README.md`, `MIGRATION.md`, and `docs/architecture.md` for broader context.

## Chunking

- Treat each feature page as one primary retrieval unit.
- Split rustdoc by public item or module, preserving the fully qualified path.
- Keep feature name, API root, target, implied features, crate version, and Git
  revision as metadata on every chunk.
- Keep migration content in a separate collection from current API guidance.
- Do not merge multiple feature pages into one undifferentiated chunk.

## Versioning

- Index documentation by both Cargo version and full Git revision.
- Never overwrite an indexed released revision with documentation from another
  revision.
- Prefer the bundle metadata produced by `scripts/build-docs-bundle.sh`.
- For an unreleased dependency pinned by `rev`, retrieve documentation with the
  same Git revision.

## Access

- Private repositories require an organization-approved Git provider
  connection, documentation crawler, or MCP server.
- Do not copy private source or documentation into an external provider without
  an approved data-handling agreement.
- Apply repository access controls to the generated documentation bundle and
  any derived vector/search index.

## Retrieval Behavior

- Resolve a requested Rust path using this deterministic order:
  1. exact match against `primary_apis`;
  2. longest matching `api_root` prefix;
  3. if multiple features share that root, return every candidate rather than
     guessing, then use exact item/rustdoc evidence to select an extension.
- Treat features that imply another feature and share/extend its API root as
  capability extensions. Examples are `distributed-id-etcd` over
  `distributed-id` and `grpc-server` over `tcp`.
- Compute the transitive closure of `implies` before presenting the final Cargo
  feature set, but show which features were requested and which were implied.
- For natural-language queries, rank `enable_when`, then `summary`, then
  `primary_apis`; return multiple candidates when scores are close.
- Return target restrictions and implied features with API guidance.
- Prefer minimal feature recommendations over broad feature sets.
- If documentation and Cargo metadata disagree, report the inconsistency and
  treat `Cargo.toml` as authoritative.

