#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

cargo fmt --all --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
