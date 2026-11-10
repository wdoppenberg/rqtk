#!/usr/bin/env bash
# Builds conformance/firesat.bundle, the git repository every client's conformance suite
# opens: the FireSat fixture with a stakeholder, a need, a consumed interface, two tests with recorded evidence,
# tag `v1` and baseline `rqtk/1.0.0`, then a commit that rewords FOBC-SW-0002 (so it is
# Suspect at HEAD). Fixed names and dates make the commits, and so the bundle, reproducible.
#
# Rerun after changing the fixture, then bless the expected results:
#   scripts/build-conformance-fixture.sh
#   RQTK_BLESS=1 cargo test -p rqtk-client --test conformance
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"
cargo build -q -p rqtk
rqtk="$repo_root/target/debug/rqtk"

work="$(mktemp -d)/firesat"
trap 'rm -rf "$(dirname "$work")"' EXIT
cp -R crates/rqtk/tests/fixtures/firesat-obc "$work"
cd "$work"

export GIT_AUTHOR_NAME=Test GIT_AUTHOR_EMAIL=test@example.com
export GIT_COMMITTER_NAME=Test GIT_COMMITTER_EMAIL=test@example.com
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
git() { command git -c init.defaultBranch=main -c commit.gpgsign=false -c tag.gpgsign=false "$@"; }
at() { export GIT_AUTHOR_DATE="$1" GIT_COMMITTER_DATE="$1"; }

mkdir -p .rqtk/stakeholders .rqtk/needs tests
printf 'id = "STK-001"\nname = "Mission operations"\n' > .rqtk/stakeholders/STK-001.toml
printf '%s\n' 'id = "NEED-0001"' 'title = "Commanding"' 'state = "Approved"' \
  'statement = "Operators need commands executed promptly."' 'stakeholders = ["STK-001"]' \
  > .rqtk/needs/NEED-0001.toml
printf '\n[trace]\nsatisfies = ["NEED-0001"]\n' >> .rqtk/requirements/SYS/FOBC-SYS-0001.toml
# A consumed interface from a provider that isn't vendored: a lint finding, and something
# for the `consumes` filter to match.
sed -i.bak 's/^parents = \["FOBC-SYS-0001"\]$/parents = ["FOBC-SYS-0001"]\nconsumes = ["ground:GS-ICD-0001"]/' \
  .rqtk/requirements/SW/FOBC-SW-0001.toml
rm .rqtk/requirements/SW/FOBC-SW-0001.toml.bak
cat > tests/test_obc.py <<'PY'
import rqtk


@rqtk.verifies("FOBC-VA-SW-001-01")
def test_commands():
    pass


@rqtk.verifies("FOBC-VA-SW-002-01")
def test_telemetry():
    pass
PY
cat > "$(dirname "$work")/junit.xml" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<testsuites><testsuite name="pytest" tests="2" failures="0">
<testcase classname="tests.test_obc" name="test_commands" file="tests/test_obc.py"/>
<testcase classname="tests.test_obc" name="test_telemetry" file="tests/test_obc.py"/>
</testsuite></testsuites>
XML
"$rqtk" verify --results "$(dirname "$work")/junit.xml" > /dev/null
# The recorded time would change every build.
sed -i.bak -E 's/^(recorded_at|executed_at) = .*/\1 = 2026-01-01T00:00:00Z/' .rqtk/evidence.toml
rm .rqtk/evidence.toml.bak

git init -q
git add -A
at "2026-01-01T00:00:00Z"
git commit -q -m "v1"
git tag v1
git tag -a rqtk/1.0.0 -m "Requirements baseline 1.0.0"
sed -i.bak 's/shall/shall always/' .rqtk/requirements/SW/FOBC-SW-0002.toml
rm .rqtk/requirements/SW/FOBC-SW-0002.toml.bak
at "2026-01-02T00:00:00Z"
git commit -q -am "v2: reword FOBC-SW-0002"

git bundle create -q "$repo_root/conformance/firesat.bundle" --all
echo "wrote conformance/firesat.bundle ($(git rev-parse --short HEAD))"
