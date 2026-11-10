"""Fixtures for the client tests: git repositories built from rqtk's own test fixtures."""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
FIRESAT = REPO / "crates/rqtk/tests/fixtures/firesat-obc"

GIT = [
    "git",
    "-c", "user.name=Test",
    "-c", "user.email=test@example.com",
    "-c", "commit.gpgsign=false",
    "-c", "tag.gpgsign=false",
    "-c", "init.defaultBranch=main",
]


def git(root: Path, *args: str) -> None:
    subprocess.run([*GIT, *args], cwd=root, check=True, capture_output=True)


def cli(root: Path, *args: str) -> object:
    """What `rqtk --json <args>` prints, parsed."""
    out = subprocess.run(
        [sys.executable, "-m", "rqtk", "--repo-root", str(root), "--json", *args],
        capture_output=True,
        text=True,
    )
    try:
        return json.loads(out.stdout)
    except json.JSONDecodeError:
        raise AssertionError(f"rqtk {' '.join(args)}: {out.stderr}") from None


@pytest.fixture(scope="session")
def firesat(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """The FireSat fixture as a git repository, with a stakeholder and a need that
    FOBC-SYS-0001 satisfies: tagged `v1` and baseline `rqtk/1.0.0`, then a second commit that
    rewords FOBC-SW-0002."""
    root = tmp_path_factory.mktemp("firesat") / "obc"
    shutil.copytree(FIRESAT, root)
    (root / ".rqtk/stakeholders").mkdir(exist_ok=True)
    (root / ".rqtk/stakeholders/STK-001.toml").write_text(
        'id = "STK-001"\nname = "Mission operations"\n'
    )
    (root / ".rqtk/needs").mkdir(exist_ok=True)
    (root / ".rqtk/needs/NEED-0001.toml").write_text(
        'id = "NEED-0001"\ntitle = "Commanding"\nstate = "Approved"\n'
        'statement = "Operators need commands executed promptly."\nstakeholders = ["STK-001"]\n'
    )
    sys1 = root / ".rqtk/requirements/SYS/FOBC-SYS-0001.toml"
    sys1.write_text(sys1.read_text() + '\n[trace]\nsatisfies = ["NEED-0001"]\n')
    git(root, "init", "-q")
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", "v1")
    git(root, "tag", "v1")
    git(root, "tag", "-a", "rqtk/1.0.0", "-m", "Requirements baseline 1.0.0")
    sw2 = root / ".rqtk/requirements/SW/FOBC-SW-0002.toml"
    sw2.write_text(sw2.read_text().replace("shall", "shall always", 1))
    git(root, "commit", "-q", "-am", "v2")
    return root
