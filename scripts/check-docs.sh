#!/usr/bin/env bash
set -euo pipefail

jq --exit-status '.schema_version == 1 and .crate.name == "hero" and (.crate.default_features | length == 0)' hero.features.json >/dev/null
node scripts/validate-feature-catalog.mjs
node scripts/generate-feature-docs.mjs --check
node scripts/generate-llms-full.mjs --check

if rg -n -i '\b(agent|agents|ai|llm|llms)\b' README.md; then
    echo "README.md must not contain agent/provider-specific language" >&2
    exit 1
fi

