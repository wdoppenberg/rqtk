#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude rqtk-py
cargo run -p rqtk-core --bin generate_schemas

if ! git diff --quiet -- schema/rqtk.schema.json schema/requirements.schema.json; then
  echo "Schema files are outdated. Regenerate with:"
  echo "  cargo run -p rqtk-core --bin generate_schemas"
  git --no-pager diff -- schema/rqtk.schema.json schema/requirements.schema.json
  exit 1
fi
