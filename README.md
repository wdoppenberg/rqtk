# rqtk

Type-driven requirements toolkit with:

- NASA-style requirement schema, validation, and traceability graph logic.
- CLI for scaffolding, linting, tracing, coverage checks, graph export, baseline updates, and basic exports.

## Workspace Layout

```text
crates/
  rqtk-core/
  rqtk/
```

## Quick Start

```bash
cargo run -p rqtk -- --help
```

## Dev Tooling

Generate TOML JSON schemas:

```bash
cargo run -p rqtk-core --bin generate_schemas
```

Install the repository-managed pre-commit hook:

```bash
./scripts/install-hooks.sh
```

Expected repository requirements directory:

```text
rqtk.toml
requirements/
  REQ-*.toml
```

## Commands

```bash
rqtk init [--requirements-dir reqs]
rqtk new --category SYS --type Performance --title "..." --statement "..." [--rationale "..."]
rqtk lint
rqtk trace REQ-SYS-0042
rqtk coverage
rqtk graph --format dot
rqtk baseline 2.5.0
rqtk export --format json|csv|markdown [--output path]
rqtk diff 2.4.0 2.5.0
```
