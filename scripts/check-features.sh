#!/usr/bin/env bash
set -euo pipefail

native_features=(
    retry
    distributed-id
    distributed-id-etcd
    shell
    tcp
    grpc-client
    grpc-server
    gateway
    s3
    smtp
    token
    native-full
)

for feature in "${native_features[@]}"; do
    cargo check --no-default-features --features "$feature"
done

cargo check --all-features

wasm_features=(retry distributed-id wasm web-full)
for feature in "${wasm_features[@]}"; do
    cargo check --target wasm32-unknown-unknown --no-default-features --features "$feature"
done

cargo check --target wasm32-unknown-unknown --all-features

