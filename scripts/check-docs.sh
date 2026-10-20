#!/usr/bin/env bash
set -euo pipefail

jq --exit-status '.schema_version == 1 and .crate.name == "midwess-hero" and .crate.lib_name == "hero" and (.crate.default_features | length == 0)' hero.features.json >/dev/null
jq --exit-status \
    '.packages[] | select(.name == "midwess-hero") | .metadata.docs.rs."all-features" == true and .metadata.docs.rs."default-target" == "x86_64-unknown-linux-gnu" and .metadata.docs.rs.targets == ["x86_64-unknown-linux-gnu"]' \
    < <(cargo metadata --no-deps --format-version 1) >/dev/null
rg -q '^#!\[deny\(missing_docs\)\]$' src/lib.rs
node scripts/validate-feature-catalog.mjs
node scripts/generate-feature-docs.mjs --check
node scripts/generate-llms-full.mjs --check

if rg -n -i '\b(agent|agents|ai|llm|llms)\b' README.md; then
    echo "README.md must not contain agent/provider-specific language" >&2
    exit 1
fi

if rg -n -i '\b(migration|migrating|migrate|migrated)\b|core-services|devlog-sdk' \
    AGENTS.md llms.txt llms-full.txt hero.features.json docs README.md; then
    echo "current public and discovery documentation must not contain consolidation-era references" >&2
    exit 1
fi

