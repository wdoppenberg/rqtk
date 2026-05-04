# rqtk

Type-driven requirements toolkit with:

- `rqtk-core`: NASA-style requirement schema, validation, and traceability graph logic.
- `rqtk`: CLI for scaffolding, linting, tracing, coverage checks, graph export, baseline updates, and basic exports.

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

Expected repository requirements directory:

```text
requirements/
  requirements.toml
  REQ-*.toml
```

## Commands

```bash
rqtk new --category SYS --type Performance --title "..." --statement "..." [--rationale "..."]
rqtk lint
rqtk trace REQ-SYS-0042
rqtk coverage
rqtk graph --format dot
rqtk baseline 2.5.0
rqtk export --format json|csv|markdown [--output path]
rqtk diff 2.4.0 2.5.0
```
