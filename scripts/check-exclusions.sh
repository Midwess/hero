#!/usr/bin/env bash
set -euo pipefail

for excluded_path in src/db src/local_storage src/services src/sdk.rs src/sdk src/utils; do
    if [[ -e "$excluded_path" ]]; then
        echo "excluded path exists: $excluded_path" >&2
        exit 1
    fi
done

if rg -n '\b(DevlogSdk|DependenciesInjection|core_services|schema::|reponse|hero::utils)' src; then
    echo "excluded compatibility symbol or dependency found" >&2
    exit 1
fi

if rg -n '^(host_machine|distributed_id|backend|gateway|grpc-client|native-full|web-full|db-red|db-idb|local-storage|zip|zstd|services)[[:space:]]*=' Cargo.toml; then
    echo "excluded legacy feature found" >&2
    exit 1
fi

if ! rg -q '^default[[:space:]]*=[[:space:]]*\[\][[:space:]]*$' Cargo.toml; then
    echo "default features must stay empty" >&2
    exit 1
fi

for feature in api-gateway distributed-id grpc-gateway logger pool retry s3 shell smtp task tcp token wasm; do
    if ! rg -q "^${feature}[[:space:]]*=" Cargo.toml; then
        echo "top-level module is missing feature: $feature" >&2
        exit 1
    fi
done

minimal_dependency_lines=$(cargo tree --quiet --no-default-features --edges normal --depth 1 | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')
if [[ "$minimal_dependency_lines" != "1" ]]; then
    echo "the empty feature set must not enable normal dependencies" >&2
    cargo tree --no-default-features --edges normal --depth 1 >&2
    exit 1
fi

while IFS= read -r feature; do
    [[ "$feature" == "default" ]] && continue
    if ! rg -Fq "| [\`${feature}\`](docs/features/${feature}.md)" README.md; then
        echo "README feature catalog is missing: $feature" >&2
        exit 1
    fi
done < <(sed -n '/^\[features\]/,/^\[/p' Cargo.toml | sed -n 's/^\([a-z0-9-]*\)[[:space:]]*=.*/\1/p')

