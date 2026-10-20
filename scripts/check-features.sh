#!/usr/bin/env bash
set -euo pipefail

mode="${1:---all}"
if [[ "$mode" != "--all" && "$mode" != "--native" && "$mode" != "--wasm" ]]; then
    echo "usage: $0 [--all|--native|--wasm]" >&2
    exit 2
fi

native_features=(
    api-gateway
    cancellation
    config
    distributed-id
    distributed-id-etcd
    env
    finally
    grpc-gateway
    logger
    maybe
    never-send
    number
    pool
    retry
    s3
    shell
    smtp
    stream
    string
    task
    tcp
    grpc-server
    throttle-future
    time
    token
    yield-container
)

if [[ "$mode" == "--all" || "$mode" == "--native" ]]; then
    for feature in "${native_features[@]}"; do
        cargo check --no-default-features --features "$feature"
    done

    cargo check --all-features
fi

wasm_features=(
    cancellation
    config
    distributed-id
    env
    finally
    logger
    maybe
    never-send
    number
    pool
    retry
    stream
    string
    throttle-future
    time
    wasm
    yield-container
)

if [[ "$mode" == "--all" || "$mode" == "--wasm" ]]; then
    for feature in "${wasm_features[@]}"; do
        cargo check --target wasm32-unknown-unknown --no-default-features --features "$feature"
    done

    wasm_all=$(IFS=,; echo "${wasm_features[*]}")
    cargo check --target wasm32-unknown-unknown --no-default-features --features "$wasm_all"
fi
