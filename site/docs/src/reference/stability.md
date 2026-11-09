# Stability

The `rqtk` command line follows [semantic versioning](https://semver.org). Within a major version, the following change only in backwards-compatible ways:

| Surface | Promise within a major version |
|---|---|
| File format (`schema_version = 1`) | Files valid today stay valid. New optional fields may be added. A breaking change gets a new `schema_version`. |
| Commands and flags | None removed or renamed. |
| `--json` output | Fields are only added, never removed, renamed or retyped. Each document's `schema` field names its major version. |
| `rqtk-client` | Its public API follows semantic versioning, released with the same version as the CLI. A test records every public item. |
| Exit codes | 0, 1, 2 and 3 keep their meaning. |
| Lint rule codes | A code keeps its meaning and is never reused. New rules may be added, so lint can find new problems after an upgrade. |
| Content hash | Tagged with its version (`v1:`); a different algorithm gets a new tag. |

**Not covered:** human-readable text output, and the Rust library crates (`rqtk-core`, `rqtk-export`, `rqtk-report`, `rqtk-macros`), which stay at 0.x and may change between minor versions, along with their re-exports from the `rqtk` crate (`rqtk::core`, `rqtk::export`, `rqtk::macros`). The `#[rqtk::verifies("…")]` attribute itself is covered.
