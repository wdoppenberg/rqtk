"""The Python client: opening repositories, querying them, and what the values look like."""

from __future__ import annotations

import dataclasses
import threading
import time
import typing
from pathlib import Path

import pytest

import rqtk
from rqtk import types as t

from conftest import cli, git


def is_default(value: object) -> bool:
    """Whether `value` is what an absent JSON field decodes to."""
    if dataclasses.is_dataclass(value):
        return all(is_default(getattr(value, f.name)) for f in dataclasses.fields(value))
    return value in (None, (), {}, False)


def assert_same(json_value: object, value: object, path: str = "$") -> None:
    """`value`, decoded from the client, holds exactly `json_value`: every JSON field under
    the same name, and nothing else except defaults for fields the JSON left out."""
    if dataclasses.is_dataclass(value):
        assert isinstance(json_value, dict), path
        names = {f.name for f in dataclasses.fields(value)}
        assert set(json_value) <= names, f"{path}: fields lost: {set(json_value) - names}"
        for name in names:
            field = getattr(value, name)
            if name in json_value:
                assert_same(json_value[name], field, f"{path}.{name}")
            else:
                assert is_default(field), f"{path}.{name} is absent from the JSON"
    elif isinstance(value, tuple):
        assert isinstance(json_value, list) and len(json_value) == len(value), path
        for i, (j, v) in enumerate(zip(json_value, value)):
            assert_same(j, v, f"{path}[{i}]")
    elif isinstance(value, dict):
        assert isinstance(json_value, dict) and set(json_value) == set(value), path
        for k in value:
            assert_same(json_value[k], value[k], f"{path}.{k}")
    else:
        assert json_value == value, f"{path}: {json_value!r} != {value!r}"


@rqtk.verifies("VA-CLIENT-0013-01")
def test_opens_a_checkout_and_lists_its_items(firesat: Path) -> None:
    repo = rqtk.open(firesat)
    assert isinstance(repo.info().origin, t.OriginCheckout)

    ids = [r.id for r in repo.requirements()]
    assert ids == sorted(ids) and "FOBC-SYS-0001" in ids
    sys1 = repo.requirement("FOBC-SYS-0001")
    assert sys1 is not None
    assert sys1.requirement.title == "On-Board Computer Command Execution"
    assert sys1.path == ".rqtk/requirements/SYS/FOBC-SYS-0001.toml"
    # Links in both directions.
    assert t.Link(kind="parents", direction="incoming", id="FOBC-SW-0001") in sys1.links
    assert t.Link(kind="satisfies", direction="outgoing", id="NEED-0001") in sys1.links
    assert repo.requirement("FOBC-SYS-9999") is None

    need = repo.need("NEED-0001")
    assert need is not None and need.satisfied
    assert [n.id for n in repo.needs()] == ["NEED-0001"]
    stakeholder = repo.stakeholder("STK-001")
    assert stakeholder is not None and stakeholder.stakeholder.name == "Mission operations"
    assert [s.id for s in repo.stakeholders()] == ["STK-001"]
    assert repo.need("NEED-9999") is None and repo.stakeholder("STK-999") is None

    activities = {a.id: a for a in repo.activities()}
    assert activities["FOBC-VA-SYS-001-01"].requirement == "FOBC-SYS-0001"
    assert isinstance(activities["FOBC-VA-SYS-001-01"], t.ActivityEntryManual)


@rqtk.verifies("VA-CLIENT-0013-01")
def test_views_hold_exactly_what_the_cli_prints(firesat: Path) -> None:
    repo = rqtk.open(firesat)
    views = [
        (["context", "FOBC-SW-0001"], repo.context("FOBC-SW-0001")),
        (["context", "NEED-0001"], repo.context("NEED-0001")),
        (["context", "STK-001"], repo.context("STK-001")),
        (["trace", "FOBC-SW-0001"], repo.trace("FOBC-SW-0001")),
        (["coverage"], repo.coverage()),
        (["lint"], repo.lint()),
        (["impact", "v1"], repo.impact("v1")),
        (["diff", "v1", "HEAD"], repo.diff("v1", "HEAD")),
        (["baseline", "--list"], repo.baselines()),
        (["log", "FOBC-SW-0002"], repo.history("FOBC-SW-0002")),
    ]
    for args, view in views:
        assert_same(cli(firesat, *args), view, " ".join(args))

    assert isinstance(repo.context("FOBC-SW-0001"), t.ContextDocRequirement)
    assert isinstance(repo.context("NEED-0001"), t.ContextDocNeed)
    assert repo.diff("v1", "HEAD").modified == (
        t.Modified(id="FOBC-SW-0002", change="semantic"),
    )
    assert [b.name for b in repo.baselines().baselines] == ["1.0.0"]
    assert len(repo.history("FOBC-SW-0002").commits) == 2


@rqtk.verifies("VA-CLIENT-0013-01")
def test_values_are_frozen_dataclasses_with_literal_enums(firesat: Path) -> None:
    entry = rqtk.open(firesat).requirements()[0]
    with pytest.raises(dataclasses.FrozenInstanceError):
        entry.id = "changed"  # type: ignore[misc]
    assert entry.status in typing.get_args(t.ClosureStatus)
    assert type(entry.status) is str
    # Plain data: asdict gives the JSON's shape.
    assert dataclasses.asdict(entry)["requirement"]["id"] == entry.id


@rqtk.verifies("VA-CLIENT-0013-01")
def test_filters_are_data(firesat: Path) -> None:
    repo = rqtk.open(firesat)
    children = repo.find_requirements(rqtk.Filter(ancestor="FOBC-SYS-0001", category=("SW",)))
    assert [r.id for r in children] == ["FOBC-SW-0001", "FOBC-SW-0002", "FOBC-SW-0003"]
    # The same filter as a dict, as it arrives from JSON.
    as_dict = repo.find_requirements({"ancestor": "FOBC-SYS-0001", "category": ["SW"]})
    assert as_dict == children
    assert repo.find_requirements() == repo.requirements()
    with pytest.raises(ValueError, match="invalid filter"):
        repo.find_requirements({"colour": ["red"]})


@rqtk.verifies("VA-CLIENT-0013-01")
def test_a_snapshot_at_a_ref_and_reload(firesat: Path, tmp_path: Path) -> None:
    at_v1 = rqtk.open(firesat, ref="v1")
    assert isinstance(at_v1.info().origin, t.OriginRef)
    assert at_v1.info().origin.reference == "v1"
    head = rqtk.open(firesat)
    reworded = head.requirement("FOBC-SW-0002").requirement.statement
    assert at_v1.requirement("FOBC-SW-0002").requirement.statement != reworded

    # A working tree snapshot sees edits only after reload().
    work = tmp_path / "work"
    git(tmp_path, "clone", "-q", str(firesat), str(work))
    repo = rqtk.open(work)
    path = work / ".rqtk/requirements/SYS/FOBC-SYS-0002.toml"
    path.write_text(path.read_text().replace('title = "', 'title = "Edited ', 1))
    assert not repo.requirement("FOBC-SYS-0002").requirement.title.startswith("Edited")
    assert repo.reload().requirement("FOBC-SYS-0002").requirement.title.startswith("Edited")


@rqtk.verifies("VA-CLIENT-0013-01")
def test_remote_repositories_open_through_the_cache(
    firesat: Path, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("RQTK_CACHE_DIR", str(tmp_path / "cache"))
    assert rqtk.cache_dir() == tmp_path / "cache"
    url = firesat.as_uri()
    with pytest.raises(rqtk.Error) as refused:
        rqtk.open_remote(url, "v1")
    assert refused.value.code == "remote_refused"

    repo = rqtk.open_remote(url, "v1", allow_file_url=True)
    assert isinstance(repo.info().origin, t.OriginRemote)
    assert repo.requirement("FOBC-SYS-0001") is not None
    assert any((tmp_path / "cache").iterdir())
    rqtk.clear_cache()
    assert not (tmp_path / "cache").exists()


@rqtk.verifies("VA-CLIENT-0013-01")
def test_errors_carry_a_code(firesat: Path, tmp_path: Path) -> None:
    def code(f: typing.Callable[[], object]) -> str:
        with pytest.raises(rqtk.Error) as raised:
            f()
        assert raised.value.code in typing.get_args(t.ErrorCode)
        assert str(raised.value) == raised.value.message
        return raised.value.code

    assert code(lambda: rqtk.open(tmp_path)) == "not_a_repository"
    broken = tmp_path / "broken"
    (broken / ".rqtk").mkdir(parents=True)
    (broken / ".rqtk/config.toml").write_text("not = [toml")
    assert code(lambda: rqtk.open(broken)) == "invalid_config"
    assert code(lambda: rqtk.open(firesat, ref="no-such-ref")) == "ref_not_found"
    repo = rqtk.open(firesat)
    assert code(lambda: repo.context("FOBC-SYS-9999")) == "not_found"
    assert code(lambda: repo.impact("no-such-ref")) == "ref_not_found"


LARGE_CONFIG = """schema_version = 1
[project]
name = "large"
version = "0.1.0"
[identification]
prefix = "BIG"
zero_padding = 4
[categories.SW]
name = "Software"
level = 1
is_root = true
[types]
allowed = ["Functional"]
[verification]
methods = ["Test"]
levels = ["System"]
phases = ["Development"]
[lifecycle]
states = ["Draft"]
default_state = "Draft"
[priority]
levels = ["High"]
[criticality]
levels = ["Non-Critical"]
[validation]
require_rationale = false
require_verification_method = true
forbid_orphans = false
forbid_circular_traces = true
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
"""


def write_large_repository(root: Path, requirements: int) -> None:
    (root / ".rqtk/requirements/SW").mkdir(parents=True)
    (root / ".rqtk/config.toml").write_text(LARGE_CONFIG)
    for i in range(1, requirements + 1):
        (root / f".rqtk/requirements/SW/BIG-SW-{i:04}.toml").write_text(
            f'id = "BIG-SW-{i:04}"\ntitle = "Requirement {i}"\ncategory = "SW"\n'
            f'type = "Functional"\nstate = "Draft"\npriority = "High"\n'
            f'statement = "The system shall do thing {i}."\n'
            f'[verification]\nmethod = "Test"\nlevel = "System"\nphase = "Development"\n'
            f'success_criteria = "Thing {i} happens."\n'
            f'[[verification.activities]]\nid = "VA-SW-{i:04}-01"\nname = "Thing {i}"\n'
        )


@rqtk.verifies("VA-CLIENT-0013-02")
def test_loading_releases_the_gil(tmp_path: Path) -> None:
    root = tmp_path / "large"
    write_large_repository(root, 2000)
    window: list[float] = []
    opened: list[rqtk.Repository] = []

    def load() -> None:
        window.append(time.perf_counter())
        opened.append(rqtk.open(root))
        window.append(time.perf_counter())

    ticks: list[float] = []
    loader = threading.Thread(target=load)
    loader.start()
    while loader.is_alive():
        ticks.append(time.perf_counter())
        time.sleep(0.001)
    loader.join()

    assert len(opened[0].requirements()) == 2000
    start, end = window
    middle = [x for x in ticks if start + (end - start) / 4 < x < end - (end - start) / 4]
    assert end - start > 0.02, "the load is too quick to tell"
    assert middle, f"no other thread ran during the {end - start:.3f} s load"
