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

# The Python package's tests, against the extension built from this checkout, in a virtual
# environment under target/ ($PYTHON picks the interpreter).
venv="target/python"
if [[ ! -x "$venv/bin/python" ]]; then
  "${PYTHON:-python3}" -m venv "$venv"
fi
"$venv/bin/python" -m pip install --quiet --disable-pip-version-check "maturin>=1.9,<2" pytest
VIRTUAL_ENV="$repo_root/$venv" "$venv/bin/maturin" develop --quiet -m crates/rqtk-py/Cargo.toml
python_junit="$(dirname "$junit")/pytest.xml"
"$venv/bin/python" -m pytest -q crates/rqtk-py/tests --junitxml "$python_junit"

# The TypeScript package's tests, against the addon built from this checkout. Node's results
# are the evidence; Bun and Deno run the same tests, and CI requires both. The tests compare
# views with the CLI's, so build it first. Remote opens use a cache under target/.
cargo build -p rqtk
export RQTK_CACHE_DIR="$repo_root/target/client-cache"
node_junit="$(dirname "$junit")/node.xml"
(
  cd crates/rqtk-node
  npm ci --silent --no-audit --no-fund
  npx napi build --platform --js native.cjs --dts native.d.ts
  npm run --silent typecheck
  node --test --test-reporter=spec --test-reporter-destination=stdout \
    --test-reporter=junit --test-reporter-destination="$repo_root/$node_junit" tests/client.test.mjs
  for runtime in bun deno; do
    if ! command -v "$runtime" > /dev/null; then
      if [[ -n "${CI:-}" ]]; then
        echo "error: $runtime is required in CI" >&2
        exit 1
      fi
      echo "warning: $runtime is not installed; skipping its run of the TypeScript tests" >&2
      continue
    fi
    case "$runtime" in
      bun) bun test tests/client.test.mjs ;;
      deno) deno test --allow-all tests/client.test.mjs ;;
    esac
  done
)

./target/debug/rqtk lint
# shellcheck disable=SC2086
./target/debug/rqtk verify $mode --results "$junit" "$python_junit" "$node_junit"

# RQTK_ALLOW_UNFINISHED: a branch built in stages may leave requirements Planned or In Progress
# (never Suspect or Failed) while later stages are open. Once nothing is unfinished the
# setting must go, so that what merges is checked strictly.
if [[ -n "${RQTK_ALLOW_UNFINISHED:-}" ]]; then
  if ./target/debug/rqtk coverage --strict > /dev/null; then
    echo "error: every requirement is Verified; remove RQTK_ALLOW_UNFINISHED from CI" >&2
    exit 1
  fi
  echo "warning: RQTK_ALLOW_UNFINISHED is set; accepting Planned and In Progress requirements" >&2
  ./target/debug/rqtk coverage --strict --allow planned --allow in-progress
else
  ./target/debug/rqtk coverage --strict
fi
