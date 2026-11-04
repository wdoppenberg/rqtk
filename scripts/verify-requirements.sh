#!/usr/bin/env bash

# Run the tests and check rqtk's own requirements against them.
#
#   scripts/verify-requirements.sh            check the committed evidence against this run
#   scripts/verify-requirements.sh --record   rewrite .rqtk/evidence.toml from this run

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

mode="--check"
if [[ "${1:-}" == "--record" ]]; then
  mode=""
fi

junit="${JUNIT_XML:-target/junit.xml}"
mkdir -p "$(dirname "$junit")"

# libtest's JUnit formatter is unstable, hence RUSTC_BOOTSTRAP; cargo-nextest is the stable
# alternative.
RUSTC_BOOTSTRAP=1 cargo test --workspace --exclude rqtk-py --tests \
  -- -Z unstable-options --format junit > "$junit"

cargo build -p rqtk
./target/debug/rqtk lint
# shellcheck disable=SC2086
./target/debug/rqtk verify $mode --results "$junit"
./target/debug/rqtk coverage --strict
