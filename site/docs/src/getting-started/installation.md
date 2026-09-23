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

## With Cargo

```bash
cargo install rqtk --locked
```

This needs Rust 1.88 or later.

## Linking tests

Linking tests to requirements needs nothing extra for most languages: a `// rqtk: verifies VA-…` comment above a test is enough. Two languages have a checked annotation:

- **Rust:** add `rqtk = { version = "1", default-features = false, features = ["macros"] }` to `[dev-dependencies]` and annotate tests with `#[rqtk::verifies("VA-…")]`. An unknown activity ID is a compile error. (The standalone `rqtk-macros` crate works too.)
- **Python:** build the extension in [`crates/rqtk-py`](https://github.com/wdoppenberg/rqtk/tree/master/crates/rqtk-py) with [maturin](https://www.maturin.rs) (a PyPI package is planned) and decorate tests with `@rqtk.verifies("VA-…")`. An unknown activity ID fails test collection. Without it, the comment tag works for Python too.

See [Verifying with tests](../guides/verification.md).

## Check the installation

```bash
rqtk --version
```
