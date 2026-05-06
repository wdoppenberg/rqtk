# rqtk

An opinionated system requirements toolkit with:

- Requirements embedded right in your repository as checked in TOML files
- NASA-style requirement schema, validation, and traceability graph logic.
- CLI for scaffolding, linting, tracing, coverage checks, graph export, baseline updates, and exports.

## Quick Start

```bash
cargo install rqtk
rqtk --help
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
rqtk codegen-cpp-verifies --output path/to/rqtk_verification_ids.hpp [--macro-name VERIFIES]
```


## Dev Tooling

Generate TOML JSON schemas:

```bash
cargo run -p rqtk-core --bin generate_schemas
```

Generate a C++ verification-ID header for compile-time test linkage:

```bash
cargo run -p rqtk -- --repo-root . codegen-cpp-verifies --output /tmp/rqtk_verification_ids.hpp
```

Install the repository-managed pre-commit hook:

```bash
./scripts/install-hooks.sh
```

See [docs/cpp-verification.md](docs/cpp-verification.md) for CMake integration and C++ usage.

Expected repository requirements directory:

```text
rqtk.toml
requirements/
  REQ-*.toml
```

