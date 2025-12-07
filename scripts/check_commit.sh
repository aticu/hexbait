#!/bin/bash

set -e

cargo fmt --all -- --check
cargo clippy --workspace --tests -- -D warnings
