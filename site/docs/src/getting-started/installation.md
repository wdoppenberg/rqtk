# Installation

## Prebuilt binaries

On macOS and Linux:

```bash
curl -LsSf https://rqtk.dev/install.sh | sh
```

On Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://rqtk.dev/install.ps1 | iex"
```

Both scripts download the right binary for your platform from the [latest GitHub release](https://github.com/wdoppenberg/rqtk/releases/latest) and verify its checksum. You can also download an archive from the release page yourself.

## With pip or uv

```bash
pip install rqtk            # or: uv tool install rqtk
```

The Python package contains the full command line and the `@rqtk.verifies` decorator for linking pytest tests. Wheels are available for Linux, macOS and Windows on Python 3.9 and later.

## With Cargo

```bash
cargo install rqtk --locked
```

This needs Rust 1.88 or later.

## Linking tests

Linking tests to requirements needs nothing extra for most languages: a `// rqtk: verifies VA-…` comment above a test is enough. Two languages have a checked annotation:

- **Rust:** add `rqtk = { version = "1", default-features = false, features = ["macros"] }` to `[dev-dependencies]` and annotate tests with `#[rqtk::verifies("VA-…")]`. An unknown activity ID is a compile error. (The standalone `rqtk-macros` crate works too.)
- **Python:** `pip install rqtk` and decorate tests with `@rqtk.verifies("VA-…")`. An unknown activity ID fails test collection.

See [Verifying with tests](../guides/verification.md).

## Check the installation

```bash
rqtk --version
```
