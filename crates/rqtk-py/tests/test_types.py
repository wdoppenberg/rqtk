"""The generated types: current with the published schemas, and named exactly as they are."""

from __future__ import annotations

import dataclasses
import importlib.util
import json
import re
import shutil
import typing
from pathlib import Path

import rqtk
from rqtk import types as t

from conftest import REPO

SCHEMA = REPO / "schema"


def generator():
    spec = importlib.util.spec_from_file_location(
        "generate_client_types", REPO / "scripts/generate-client-types.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def stale(schema_dir: Path) -> list[Path]:
    """The committed generated files that differ from what `schema_dir` generates."""
    return [path for path, text in generator().generated(schema_dir).items()
            if not path.exists() or path.read_text() != text]


@rqtk.verifies("VA-CLIENT-0016-01")
def test_committed_types_are_current() -> None:
    assert stale(SCHEMA) == [], "run scripts/generate-client-types.py"


@rqtk.verifies("VA-CLIENT-0016-01")
def test_a_changed_client_type_fails_the_check(tmp_path: Path) -> None:
    changed = tmp_path / "schema"
    shutil.copytree(SCHEMA, changed)
    path = changed / "client/requirement-entry.schema.json"
    schema = json.loads(path.read_text())
    schema["properties"]["owner"] = {"type": "string"}
    path.write_text(json.dumps(schema))
    assert stale(changed) != []


def schemas() -> dict[str, dict]:
    """Every named type in the client's schemas: the roots, by title, and their `$defs`."""
    named: dict[str, dict] = {}
    gen = generator()
    files = [SCHEMA / "client" / f"{n}.schema.json" for n in gen.CLIENT]
    files += [SCHEMA / "output" / f"{n}.schema.json" for n in gen.VIEWS]
    for path in files:
        doc = json.loads(path.read_text())
        named.update(doc.get("$defs", {}))
        if "client-error" not in path.name:
            named[doc["title"]] = doc
    return named


def properties(schema: dict, named: dict[str, dict]) -> set[str]:
    props = set(schema.get("properties", {}))
    if "$ref" in schema:
        props |= properties(named[schema["$ref"].split("/")[-1]], named)
    return props


def field_names(cls: type) -> set[str]:
    return {f.name for f in dataclasses.fields(cls)}


def typescript_interfaces() -> dict[str, set[str]]:
    """Each interface in the generated `types.d.ts` and its fields."""
    text = (REPO / "crates/rqtk-node/types.d.ts").read_text()
    interfaces = {}
    for name, body in re.findall(r"^export interface (\w+) \{\n(.*?)^\}", text, re.M | re.S):
        interfaces[name] = set(re.findall(r"^  readonly (\w+)\??:", body, re.M))
    return interfaces


@rqtk.verifies("VA-CLIENT-0015-01")
def test_field_names_are_the_schema_property_names() -> None:
    named = schemas()
    typescript = typescript_interfaces()
    checked = 0

    def check(cls: type, expected: set[str]) -> None:
        nonlocal checked
        assert field_names(cls) == expected, f"Python {cls.__name__}"
        assert typescript.pop(cls.__name__) == expected, f"TypeScript {cls.__name__}"
        checked += 1

    for name, schema in named.items():
        generated = getattr(t, name)
        if "oneOf" in schema and all("properties" in v for v in schema["oneOf"]):
            # A tagged union: one dataclass per variant, with the union's own properties.
            variants = typing.get_args(generated)
            assert len(variants) == len(schema["oneOf"]), name
            for variant, variant_schema in zip(variants, schema["oneOf"]):
                check(variant, properties(schema, named) | properties(variant_schema, named))
        elif schema.get("type") == "object" and "properties" in schema:
            check(generated, properties(schema, named))
    assert checked > 50
    assert typescript == {}, f"TypeScript interfaces without a schema: {sorted(typescript)}"

    # The error is an exception, with the fields of the client's error.
    error = json.loads((SCHEMA / "client/client-error.schema.json").read_text())
    raised = rqtk.Error("not_found", "nothing")
    assert {name for name in error["properties"] if hasattr(raised, name)} == set(
        error["properties"]
    )
    assert typing.get_args(t.ErrorCode) == tuple(
        v["const"] for v in error["$defs"]["ErrorCode"]["oneOf"]
    )
