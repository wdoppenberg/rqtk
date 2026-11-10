"""The conformance suite in Python: each case in `conformance/cases.json`, compared with its
`conformance/expected/<name>.json`. Rust writes those files and TypeScript checks the same
ones; see `conformance/README.md`.

The Python client returns dataclasses, so results are turned back into JSON to compare. A
field the JSON leaves out decodes to None, (), {} or False, so those count as absent.
"""

from __future__ import annotations

import dataclasses
import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any, Callable, Iterator

import pytest

import rqtk

from conftest import REPO

CONFORMANCE = REPO / "conformance"
SUITE = json.loads((CONFORMANCE / "cases.json").read_text())
CASES = SUITE["cases"]


@pytest.fixture(scope="module")
def fixtures(tmp_path_factory: pytest.TempPathFactory) -> Iterator[Path]:
    root = tmp_path_factory.mktemp("conformance")
    for name, spec in SUITE["fixtures"].items():
        target = root / name
        if "bundle" in spec:
            subprocess.run(["git", "clone", "-q", str(REPO / spec["bundle"]), str(target)],
                           check=True, capture_output=True)
        elif "copy" in spec:
            shutil.copytree(REPO / spec["copy"], target)
        else:
            target.mkdir()
            for rel, text in spec.get("files", {}).items():
                (target / rel).parent.mkdir(parents=True, exist_ok=True)
                (target / rel).write_text(text)
            for rel in spec.get("directories", []):
                (target / rel).mkdir(parents=True)
    before = os.environ.get("RQTK_CACHE_DIR")
    os.environ["RQTK_CACHE_DIR"] = str(root / "cache")
    yield root
    if before is None:
        del os.environ["RQTK_CACHE_DIR"]
    else:
        os.environ["RQTK_CACHE_DIR"] = before


def file_url(path: Path) -> str:
    text = str(path).replace("\\", "/")
    return f"file://{text}" if text.startswith("/") else f"file:///{text}"


def opened(root: Path, spec: dict[str, Any]) -> rqtk.Repository:
    if "fixture" in spec:
        return rqtk.open(root / spec["fixture"], ref=spec.get("ref"))
    if "remote" in spec:
        url = file_url(root / spec["remote"])
        return rqtk.open_remote(url, spec["ref"], allow_file_url=spec.get("allow_file_url", True))
    return rqtk.open_remote(spec["url"], spec["ref"])


def call(repo: rqtk.Repository, case: dict[str, Any]) -> Any:
    method: Callable[..., Any] = getattr(repo, case["call"])
    return method(*case["args"])


def plain(value: Any) -> Any:
    """A client value as JSON, with absent-equivalent fields dropped."""
    if dataclasses.is_dataclass(value) and not isinstance(value, type):
        value = {f.name: getattr(value, f.name) for f in dataclasses.fields(value)}
    if isinstance(value, dict):
        out = {k: plain(v) for k, v in value.items()}
        return {k: v for k, v in out.items() if v not in (None, [], {}) and v is not False}
    if isinstance(value, (list, tuple)):
        return [plain(v) for v in value]
    return value


def run(root: Path, case: dict[str, Any]) -> dict[str, Any]:
    try:
        return {"result": plain(call(opened(root, case["open"]), case))}
    except rqtk.Error as error:
        return {"error": error.code}


def expected(root: Path, case: dict[str, Any]) -> dict[str, Any]:
    text = (CONFORMANCE / "expected" / f"{case['name']}.json").read_text()
    spec = case["open"]
    name = spec.get("fixture") or spec.get("remote")
    escaped = lambda s: json.dumps(s)[1:-1]  # noqa: E731
    if name:
        text = text.replace("$URL", escaped(file_url(root / name)))
        text = text.replace("$ROOT", escaped(str(root / name)))
    doc = json.loads(text)
    return {"result": plain(doc["result"])} if "result" in doc else doc


def is_error(case: dict[str, Any]) -> bool:
    return case["name"].startswith("error-")


def is_filter(case: dict[str, Any]) -> bool:
    return case["call"] == "find_requirements"


def check(root: Path, select: Callable[[dict[str, Any]], bool]) -> None:
    cases = [c for c in CASES if select(c)]
    assert cases
    failures = [c["name"] for c in cases if run(root, c) != expected(root, c)]
    assert failures == [], f"differ from conformance/expected: {failures}"


@rqtk.verifies("VA-CLIENT-0017-02")
@rqtk.verifies("VA-SYS-0007-01")
def test_every_view_matches_the_suite(fixtures: Path) -> None:
    check(fixtures, lambda c: not is_error(c) and not is_filter(c))


@rqtk.verifies("VA-CLIENT-0011-02")
def test_every_filter_matches_the_suite(fixtures: Path) -> None:
    check(fixtures, is_filter)


@rqtk.verifies("VA-CLIENT-0009-01")
def test_every_error_code_matches_the_suite(fixtures: Path) -> None:
    check(fixtures, is_error)


def test_absent_fields_are_not_hidden_by_the_comparison(fixtures: Path) -> None:
    """`plain` drops empty fields only; a case's content still has to match."""
    case = next(c for c in CASES if c["name"] == "requirement")
    result = run(fixtures, case)
    assert result["result"]["requirement"]["title"]
    assert result != run(fixtures, {**case, "args": ["FOBC-SYS-0002"]})
