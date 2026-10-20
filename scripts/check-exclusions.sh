#!/usr/bin/env bash
set -euo pipefail

for excluded_path in src/db src/local_storage src/services src/sdk.rs src/sdk; do
    if [[ -e "$excluded_path" ]]; then
        echo "excluded path exists: $excluded_path" >&2
        exit 1
    fi
done

if rg -n '\b(DevlogSdk|DependenciesInjection|core_services|schema::|reponse)' src; then
    echo "excluded compatibility symbol or dependency found" >&2
    exit 1
fi

if rg -n '^(host_machine|distributed_id|backend|db-red|db-idb|local-storage|zip|zstd|services)[[:space:]]*=' Cargo.toml; then
    echo "excluded legacy feature found" >&2
    exit 1
fi

