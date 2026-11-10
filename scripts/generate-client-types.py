#!/usr/bin/env python3
"""Generate the client's data types from its published JSON Schemas.

    scripts/generate-client-types.py           write the generated files
    scripts/generate-client-types.py --check   exit 1 if a committed file is out of date

Reads `schema/client/` and the view documents in `schema/output/`, and writes

- `crates/rqtk-py/python/rqtk/types.py`: a frozen dataclass per object, a `Literal` per string
  enum, a `Union` of dataclasses per tagged union, and a decoder per type that builds them
  from the client's JSON;
- `crates/rqtk-node/types.d.ts`: a readonly interface per object, a string union per enum
  and a union of interfaces per tagged union, describing the client's JSON as it is.

Field names are the schema's property names, unchanged, in both.

Standard library only, Python 3.9 or later.
"""

from __future__ import annotations

import json
import keyword
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCHEMA = ROOT / "schema"
PYTHON_OUT = ROOT / "crates/rqtk-py/python/rqtk/types.py"
TYPESCRIPT_OUT = ROOT / "crates/rqtk-node/types.d.ts"

# The client's data types and the views it returns. `client-error` contributes its `ErrorCode`
# only: the error itself is the `rqtk.Error` exception.
CLIENT = ["requirement-entry", "need-entry", "stakeholder-entry", "activity-entry",
          "repository-info", "filter", "client-error"]
VIEWS = ["context", "trace", "coverage", "lint", "impact", "diff", "baselines", "history"]
ROOTLESS = {"client-error"}
# Types callers build (and the client deserializes): every field has a default.
INPUTS = {"Filter"}


class SchemaError(Exception):
    pass


# ---------------------------------------------------------------------------------------------
# The type model: what each named schema is. Shared by every target language.


def load(schema_dir: Path = SCHEMA) -> dict[str, dict]:
    """Every named type, by name: each file's root (by its `title`) and its `$defs`."""
    named: dict[str, dict] = {}
    sources: dict[str, str] = {}

    def add(name: str, schema: dict, source: str) -> None:
        if name in named and named[name] != schema:
            raise SchemaError(f"`{name}` differs between {sources[name]} and {source}")
        named.setdefault(name, schema)
        sources.setdefault(name, source)

    files = [schema_dir / "client" / f"{n}.schema.json" for n in CLIENT]
    files += [schema_dir / "output" / f"{n}.schema.json" for n in VIEWS]
    for path in files:
        doc = json.loads(path.read_text())
        rel = str(path.relative_to(schema_dir))
        for name, schema in doc.get("$defs", {}).items():
            add(name, schema, rel)
        if path.name.removesuffix(".schema.json") not in ROOTLESS:
            root = {k: v for k, v in doc.items() if k not in ("$defs", "$schema", "$id", "title")}
            add(doc["title"], root, rel)
    return named


def ref_name(schema: dict) -> str | None:
    ref = schema.get("$ref")
    if ref is None:
        return None
    prefix = "#/$defs/"
    if not ref.startswith(prefix):
        raise SchemaError(f"unsupported $ref {ref}")
    return ref[len(prefix):]


def string_values(schema: dict) -> list[str] | None:
    """The values of a string enum (`enum`, `const`, or a `oneOf` of those), else None."""
    if "const" in schema and isinstance(schema["const"], str):
        return [schema["const"]]
    if "enum" in schema and all(isinstance(v, str) for v in schema["enum"]):
        return list(schema["enum"])
    if "oneOf" in schema:
        values = []
        for option in schema["oneOf"]:
            part = string_values(option)
            if part is None:
                return None
            values += part
        return values
    return None


def discriminator(variants: list[dict]) -> str:
    """The property that is a string const in every variant of a tagged union."""
    candidates = None
    for v in variants:
        tags = {k for k, p in v.get("properties", {}).items() if isinstance(p.get("const"), str)}
        candidates = tags if candidates is None else candidates & tags
    if not candidates or len(candidates) != 1:
        raise SchemaError(f"no single tag property in union variants: {candidates}")
    return candidates.pop()


def description(schema: dict | bool) -> str | None:
    return schema.get("description") if isinstance(schema, dict) else None


def camel(text: str) -> str:
    return "".join(part[:1].upper() + part[1:] for part in text.replace("-", "_").split("_"))


def kind(schema: dict) -> str:
    """`literal`, `alias` (a plain string or map), `union` (tagged) or `object`."""
    if string_values(schema) is not None:
        return "literal"
    if "oneOf" in schema:
        return "union"
    if schema.get("type") == "object" and "properties" in schema:
        return "object"
    return "alias"


def union_variants(
    named: dict[str, dict], name: str, schema: dict
) -> tuple[str, list[tuple[str, dict]]]:
    """A tagged union's tag and its variants as `(class name, object schema)`. Properties of
    the union itself (`ActivityStatus.id`) are part of every variant, and so are those of a
    type a variant refers to (`ContextDoc`'s `RequirementContext`)."""
    variants = schema["oneOf"]
    tag = discriminator(variants)
    shared = schema.get("properties", {})
    shared_required = schema.get("required", [])
    out = []
    for v in variants:
        base = named[ref_name(v)] if "$ref" in v else {}
        if base and kind(base) != "object":
            raise SchemaError(f"variant of `{name}` refers to a non-object")
        props = {**shared, **base.get("properties", {}), **v["properties"]}
        required = shared_required + base.get("required", []) + v.get("required", [])
        required = list(dict.fromkeys(required))
        merged = {"type": "object", "properties": props, "required": required}
        if "description" in v:
            merged["description"] = v["description"]
        out.append((name + camel(v["properties"][tag]["const"]), merged))
    return tag, out


# ---------------------------------------------------------------------------------------------
# Python


PRIMITIVES = {"string": "str", "integer": "int", "number": "float", "boolean": "bool"}


class Python:
    def __init__(self, named: dict[str, dict]):
        self.named = named
        self.lines: list[str] = []

    # Type expressions -----------------------------------------------------------------------

    def annotation(self, schema: dict | bool) -> str:
        if schema is True or schema == {}:
            return "Any"
        name = ref_name(schema)
        if name is not None:
            return name
        values = string_values(schema)
        if values is not None:
            return "Literal[" + ", ".join(json.dumps(v) for v in values) + "]"
        if "anyOf" in schema:
            options = [o for o in schema["anyOf"] if o.get("type") != "null"]
            if len(options) != 1 or len(options) == len(schema["anyOf"]):
                raise SchemaError(f"unsupported anyOf {schema}")
            return f"Optional[{self.annotation(options[0])}]"
        types = schema.get("type")
        if isinstance(types, list):
            rest = [t for t in types if t != "null"]
            if len(rest) != 1:
                raise SchemaError(f"unsupported type list {types}")
            inner = self.annotation({**schema, "type": rest[0]})
            return f"Optional[{inner}]" if "null" in types else inner
        if types in PRIMITIVES:
            return PRIMITIVES[types]
        if types == "array":
            if "prefixItems" in schema:
                return "Tuple[" + ", ".join(self.annotation(i) for i in schema["prefixItems"]) + "]"
            return f"Tuple[{self.annotation(schema['items'])}, ...]"
        if types == "object" and schema.get("additionalProperties") not in (None, False):
            return f"Dict[str, {self.annotation(schema['additionalProperties'])}]"
        raise SchemaError(f"unsupported schema {schema}")

    def decoder(self, schema: dict, value: str) -> str:
        """An expression that decodes JSON `value` into the type of `schema`."""
        if not self.needs_decoding(schema):
            return value
        name = ref_name(schema)
        if name is not None:
            return f"_decode_{name}({value})" if self.needs_decoding(schema) else value
        if "anyOf" in schema:
            inner = next(o for o in schema["anyOf"] if o.get("type") != "null")
            return self.optional(inner, value)
        types = schema.get("type")
        if isinstance(types, list):
            rest = [t for t in types if t != "null"]
            return self.optional({**schema, "type": rest[0]}, value)
        if types == "array":
            if "prefixItems" in schema:
                return f"tuple({value})"
            item = schema["items"]
            if not self.needs_decoding(item):
                return f"tuple({value})"
            return f"tuple({self.decoder(item, 'x')} for x in {value})"
        if types == "object" and schema.get("additionalProperties") not in (None, False):
            item = schema["additionalProperties"]
            if not self.needs_decoding(item):
                return f"dict({value})"
            return f"{{k: {self.decoder(item, 'x')} for k, x in {value}.items()}}"
        return value

    def optional(self, inner: dict, value: str) -> str:
        if not self.needs_decoding(inner):
            return value
        return f"(None if {value} is None else {self.decoder(inner, value)})"

    def needs_decoding(self, schema: dict) -> bool:
        """Whether JSON for `schema` needs more than itself to become the Python value."""
        if not isinstance(schema, dict):
            return False
        name = ref_name(schema)
        if name is not None:
            target = self.named[name]
            k = kind(target)
            return k in ("object", "union") or (k == "alias" and self.needs_decoding(target))
        if "anyOf" in schema:
            return any(self.needs_decoding(o) for o in schema["anyOf"])
        types = schema.get("type")
        return types == "array" or (
            types == "object" and schema.get("additionalProperties") not in (None, False)
        ) or (isinstance(types, list) and any(t in ("array", "object") for t in types))

    # Fields ---------------------------------------------------------------------------------

    def absent(self, schema: dict | bool) -> tuple[str, str] | None:
        """For a property the JSON may leave out: its annotation and the JSON to read when it
        is absent (serde's default), or None if absent means None."""
        if not isinstance(schema, dict):
            return None
        name = ref_name(schema)
        target = self.named[name] if name is not None else schema
        types = target.get("type")
        if types == "array":
            return self.annotation(schema), "()"
        if types == "object" and "properties" not in target and target.get("additionalProperties"):
            return self.annotation(schema), "{}"
        if types == "boolean":
            return self.annotation(schema), "False"
        if name is not None and kind(target) == "object" and not target.get("required"):
            return self.annotation(schema), "{}"
        return None

    def fields(self, schema: dict, defaults: bool) -> list[tuple[str, str, str, str | None]]:
        """`(name, annotation, decoder, description)` for each property."""
        required = set(schema.get("required", []))
        out = []
        for prop, sub in schema["properties"].items():
            if keyword.iskeyword(prop) or not prop.isidentifier():
                raise SchemaError(f"property `{prop}` is not a Python identifier")
            annotation = self.annotation(sub)
            if prop in required:
                value = f"d[{prop!r}]"
            else:
                absent = self.absent(sub)
                if absent is None:
                    if not annotation.startswith("Optional["):
                        annotation = f"Optional[{annotation}]"
                    value = f"d.get({prop!r})"
                else:
                    annotation = absent[0]
                    value = f"d.get({prop!r}, {absent[1]})"
            default = None
            if defaults:
                default = "()" if annotation.startswith("Tuple[") else "None"
                if not (annotation.startswith(("Tuple[", "Optional["))):
                    raise SchemaError(f"input field `{prop}` has no default")
            decode = self.optional(sub, value) if value == f"d.get({prop!r})" else self.decoder(
                sub, value
            )
            out.append((prop, annotation, decode, description(sub), default))
        return out

    # Output ---------------------------------------------------------------------------------

    def emit(self, text: str = "") -> None:
        self.lines.append(text)

    def docstring(self, text: str | None, indent: str) -> None:
        if not text:
            return
        text = text.replace("\\", "\\\\").replace('"""', '\\"\\"\\"')
        lines = text.strip().split("\n")
        if len(lines) == 1:
            self.emit(f'{indent}"""{lines[0]}"""')
        else:
            self.emit(f'{indent}"""{lines[0]}')
            for line in lines[1:]:
                self.emit(f"{indent}{line}".rstrip())
            self.emit(f'{indent}"""')

    def dataclass(self, name: str, schema: dict) -> None:
        self.emit()
        self.emit()
        self.emit("@dataclass(frozen=True)")
        self.emit(f"class {name}:")
        self.docstring(schema.get("description"), "    ")
        fields = self.fields(schema, defaults=name in INPUTS)
        for prop, annotation, _, description, default in fields:
            self.emit(f"    {prop}: {annotation}" + (f" = {default}" if default else ""))
            self.docstring(description, "    ")
        if not fields:
            self.emit("    pass")
        self.emit()
        self.emit()
        self.emit(f"def _decode_{name}(d: Any) -> {name}:")
        self.emit(f"    return {name}(")
        for prop, _, decode, _, _ in fields:
            self.emit(f"        {prop}={decode},")
        self.emit("    )")

    def generate(self) -> str:
        literals, aliases, objects, unions = [], [], [], []
        for name in sorted(self.named):
            schema = self.named[name]
            k = kind(schema)
            {"literal": literals, "alias": aliases, "object": objects, "union": unions}[k].append(
                name
            )
        self.emit('"""The client\'s data types, generated from its JSON Schemas.')
        self.emit()
        self.emit("Generated by scripts/generate-client-types.py: do not edit. Field names are the")
        self.emit("JSON's; `dataclasses.asdict` turns any value back into that JSON's shape.")
        self.emit('"""')
        self.emit()
        self.emit("from __future__ import annotations")
        self.emit()
        self.emit("from dataclasses import dataclass")
        self.emit("from typing import Any, Callable, Dict, Literal, Optional, Tuple, Union")
        exported = []
        for name in literals:
            schema = self.named[name]
            self.emit()
            self.emit(f"{name} = {self.annotation(schema)}")
            self.docstring(schema.get("description"), "")
            exported.append(name)
        for name in aliases:
            schema = self.named[name]
            self.emit()
            self.emit(f"{name} = {self.annotation(schema)}")
            self.docstring(schema.get("description"), "")
            exported.append(name)
        for name in objects:
            self.dataclass(name, self.named[name])
            exported.append(name)
        for name in unions:
            tag, variants = union_variants(self.named, name, self.named[name])
            for variant, schema in variants:
                self.dataclass(variant, schema)
                exported.append(variant)
            self.emit()
            self.emit()
            self.emit(f"{name} = Union[" + ", ".join(v for v, _ in variants) + "]")
            self.docstring(self.named[name].get("description"), "")
            self.emit()
            self.emit(f"_{name}_VARIANTS: Dict[str, Callable[[Any], {name}]] = {{")
            for variant, schema in variants:
                const = schema["properties"][tag]["const"]
                self.emit(f"    {const!r}: _decode_{variant},")
            self.emit("}")
            self.emit()
            self.emit()
            self.emit(f"def _decode_{name}(d: Any) -> {name}:")
            self.emit(f"    return _{name}_VARIANTS[d[{tag!r}]](d)")
            exported.append(name)
        self.emit()
        self.emit("__all__ = [")
        for name in sorted(exported):
            self.emit(f"    {name!r},")
        self.emit("]")
        return "\n".join(self.lines) + "\n"

# ---------------------------------------------------------------------------------------------
# TypeScript


TS_PRIMITIVES = {"string": "string", "integer": "number", "number": "number", "boolean": "boolean"}


class TypeScript:
    def __init__(self, named: dict[str, dict]):
        self.named = named
        self.lines: list[str] = []

    def type(self, schema: dict | bool) -> str:
        if schema is True or schema == {}:
            return "unknown"
        name = ref_name(schema)
        if name is not None:
            return name
        values = string_values(schema)
        if values is not None:
            return " | ".join(json.dumps(v) for v in values)
        if "anyOf" in schema:
            options = [o for o in schema["anyOf"] if o.get("type") != "null"]
            if len(options) != 1 or len(options) == len(schema["anyOf"]):
                raise SchemaError(f"unsupported anyOf {schema}")
            return f"{self.type(options[0])} | null"
        types = schema.get("type")
        if isinstance(types, list):
            rest = [t for t in types if t != "null"]
            if len(rest) != 1:
                raise SchemaError(f"unsupported type list {types}")
            inner = self.type({**schema, "type": rest[0]})
            return f"{inner} | null" if "null" in types else inner
        if types in TS_PRIMITIVES:
            return TS_PRIMITIVES[types]
        if types == "array":
            if "prefixItems" in schema:
                return "readonly [" + ", ".join(self.type(i) for i in schema["prefixItems"]) + "]"
            item = self.type(schema["items"])
            return f"readonly ({item})[]" if " " in item else f"readonly {item}[]"
        if types == "object" and schema.get("additionalProperties") not in (None, False):
            return f"Readonly<Record<string, {self.type(schema['additionalProperties'])}>>"
        raise SchemaError(f"unsupported schema {schema}")

    def emit(self, text: str = "") -> None:
        self.lines.append(text)

    def doc(self, text: str | None, indent: str) -> None:
        if not text:
            return
        lines = text.strip().replace("*/", "*\\/").split("\n")
        if len(lines) == 1:
            self.emit(f"{indent}/** {lines[0]} */")
            return
        self.emit(f"{indent}/**")
        for line in lines:
            self.emit(f"{indent} * {line}".rstrip())
        self.emit(f"{indent} */")

    def interface(self, name: str, schema: dict) -> None:
        self.emit()
        self.doc(schema.get("description"), "")
        self.emit(f"export interface {name} {{")
        required = set(schema.get("required", []))
        for prop, sub in schema["properties"].items():
            if not prop.isidentifier():
                raise SchemaError(f"property `{prop}` is not an identifier")
            self.doc(description(sub), "  ")
            optional = "" if prop in required else "?"
            self.emit(f"  readonly {prop}{optional}: {self.type(sub)};")
        self.emit("}")

    def generate(self) -> str:
        self.emit("// The client's data types, generated from its JSON Schemas.")
        self.emit("//")
        self.emit("// Generated by scripts/generate-client-types.py: do not edit. Each type describes")
        self.emit("// the client's JSON as it is, with the JSON's field names; a field marked `?` is")
        self.emit("// left out of the JSON when it is empty, false or unset.")
        for name in sorted(self.named):
            schema = self.named[name]
            k = kind(schema)
            if k in ("literal", "alias"):
                self.emit()
                self.doc(schema.get("description"), "")
                self.emit(f"export type {name} = {self.type(schema)};")
            elif k == "object":
                self.interface(name, schema)
            else:
                _, variants = union_variants(self.named, name, schema)
                for variant, variant_schema in variants:
                    self.interface(variant, variant_schema)
                self.emit()
                self.doc(schema.get("description"), "")
                self.emit(f"export type {name} = " + " | ".join(v for v, _ in variants) + ";")
        return "\n".join(self.lines) + "\n"


def generated(schema_dir: Path = SCHEMA) -> dict[Path, str]:
    """Each generated file's path and content, from the schemas in `schema_dir`."""
    named = load(schema_dir)
    return {
        PYTHON_OUT: Python(named).generate(),
        TYPESCRIPT_OUT: TypeScript(named).generate(),
    }


def main(argv: list[str]) -> int:
    check = "--check" in argv[1:]
    try:
        outputs = generated()
    except SchemaError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    stale = []
    for path, text in outputs.items():
        current = path.read_text() if path.exists() else None
        if current == text:
            continue
        if check:
            stale.append(path)
        else:
            path.write_text(text)
            print(f"wrote {path.relative_to(ROOT)}")
    for path in stale:
        print(f"{path.relative_to(ROOT)} is out of date; run scripts/generate-client-types.py",
              file=sys.stderr)
    return 1 if stale else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
