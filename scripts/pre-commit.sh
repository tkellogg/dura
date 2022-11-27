#!/bin/sh
# This script is intended to be run before committing. It represents what's don in CI anyway, 
# but should reduce the frustration of getting your commits rejected. This isn't identical to 
# what happens in CI, it's a much faster version, it does everything in DEBUG.

# Failed commands should cause the entire script to fail immediately
set -e

echo "################################"
echo "### cargo test"
echo "################################"
cargo test

echo "################################"
echo "### cargo clippy"
echo "################################"
cargo clippy --all-targets --all-features -- -D warnings

echo "################################"
echo "### cargo fmt"
echo "################################"
# This doesn't fail, it just leaves files changed
cargo fmt
if [[ ! -z "$(git diff-index --name-only HEAD --)" ]]; then
  echo "Error: Git changes present, maybe from 'cargo fmt', consider committing"
  exit 1
fi
