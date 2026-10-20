#!/usr/bin/env bash
set -euo pipefail

scripts/check-docs.sh

version=$(cargo metadata --no-deps --format-version 1 | jq --raw-output '.packages[] | select(.name == "hero") | .version')
revision=$(git rev-parse HEAD)
revision_label="$revision"
distributable=true
if [[ -n "$(git status --porcelain)" ]]; then
    if [[ "${ALLOW_DIRTY_DOCS:-0}" != "1" ]]; then
        echo "refusing to build a distributable documentation bundle from a dirty worktree" >&2
        echo "commit the documentation, or set ALLOW_DIRTY_DOCS=1 for a local non-distributable preview" >&2
        exit 1
    fi
    revision_label="${revision}-dirty"
    distributable=false
fi

wasm_features="cancellation,config,distributed-id,env,finally,logger,maybe,never-send,number,pool,retry,stream,string,throttle-future,time,wasm,yield-container"
rm -rf target/doc target/wasm32-unknown-unknown/doc
cargo doc --no-deps --all-features
cargo doc --no-deps --target wasm32-unknown-unknown --no-default-features --features "$wasm_features"

bundle_root="target/docs-bundle/${version}/${revision_label}"
rm -rf "$bundle_root"
mkdir -p "$bundle_root/source" "$bundle_root/rustdoc/native" "$bundle_root/rustdoc/wasm"

cp README.md MIGRATION.md HISTORY.md AGENTS.md Cargo.toml llms.txt llms-full.txt hero.features.json "$bundle_root/source/"
cp -R docs "$bundle_root/source/docs"
cp -R target/doc/. "$bundle_root/rustdoc/native/"
cp -R target/wasm32-unknown-unknown/doc/. "$bundle_root/rustdoc/wasm/"

created_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --null-input \
    --arg crate "hero" \
    --arg version "$version" \
    --arg revision "$revision" \
    --arg revision_label "$revision_label" \
    --arg created_at "$created_at" \
    --argjson distributable "$distributable" \
    '{crate: $crate, version: $version, revision: $revision, revision_label: $revision_label, created_at: $created_at, distributable: $distributable, default_features: [], catalog_schema: 1}' \
    > "$bundle_root/bundle-metadata.json"

archive="target/docs-bundle/hero-docs-${version}-${revision_label}.tar.gz"
tar -czf "$archive" -C "target/docs-bundle/${version}" "$revision_label"
printf '%s\n' "$archive"

