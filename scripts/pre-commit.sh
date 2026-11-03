#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude rqtk-py --doc

# Ensure content hashes are current, then run the tests and check the project requirements:
# every file lints, the committed evidence matches this run, and everything is verified.
cargo run -p rqtk -- --repo-root . rehash
scripts/verify-requirements.sh

cargo run -p rqtk-core --bin generate_schemas

if ! git diff --quiet -- schema/; then
  echo "Schema files are outdated. Regenerate with:"
  echo "  cargo run -p rqtk-core --bin generate_schemas"
  git --no-pager diff -- schema/
  exit 1
fi
