# Hero Repository Instructions

Before modifying Hero, read these files in order:

1. `llms.txt`
2. `hero.features.json`
3. the relevant page under `docs/features/`
4. `Cargo.toml`

Repository invariants:

- `default = []` must remain empty.
- Every public module must have an explicit Cargo feature.
- Every normal dependency must be optional and owned by a feature.
- Do not reintroduce `utils`, `db`, `local_storage`, `services`, or `sdk` modules.
- Do not add broad production presets such as `full` or `backend`.
- Native-only features must remain unavailable on WASM.
- Feature relationships belong in `Cargo.toml` and `hero.features.json`.
- Keep README content product-facing; discovery-system instructions belong in
  `llms.txt`, `AGENTS.md`, and `docs/indexing.md`.

When adding or changing a feature:

1. Update `Cargo.toml`.
2. Update `hero.features.json`.
3. Run `node scripts/generate-feature-docs.mjs`.
4. Update source and tests.
5. Run `scripts/check-docs.sh`, `scripts/check-features.sh`, and
   `scripts/check-exclusions.sh`.
6. Run native and supported-WASM Clippy checks.

When releasing Hero:

1. Increase the stable semantic version in `Cargo.toml`.
2. Set the same version in `hero.features.json`.
3. Regenerate feature pages and `llms-full.txt`.
4. Push to `main`; `.github/workflows/release.yml` publishes the crate and
   creates the matching GitHub release after validation.

