# Verifying with tests

A requirement is **verified** only by recorded test results, never by editing a file. rqtk records which tests passed against which version of each requirement. When the requirement changes afterwards, that evidence turns **Suspect** until the tests run again.

## The loop

1. **Link a test** to a verification activity.
2. **Run the tests** with JUnit XML output.
3. **Record the results** with `rqtk verify`.
4. **Check** with `rqtk coverage --strict`, and commit `.rqtk/evidence.toml` with the code.

## Linking tests

Put the activity ID directly above the test function.

| Language | Annotation | Checked when |
|---|---|---|
| Rust | `#[rqtk_macros::verifies("VA-…")]` | compile time |
| Python | `@rqtk.verifies("VA-…")` | test collection |
| Anything | `// rqtk: verifies VA-…` (or `#`, `--`) | `rqtk lint` |

A test can verify several activities, and an activity can have several tests; it passes only when all of them pass. `rqtk scan` lists every link, and `rqtk lint` reports links to unknown activities (RQ028) and annotations with no function below them (RQ030).

Choose which files are scanned in `.rqtk/config.toml`:

```toml
[scan]
paths = ["."]                        # default; .gitignore is honoured
exclude = ["tests/fixtures/**"]      # gitignore-style globs
```

## Running tests with JUnit output

rqtk reads [JUnit XML](../reference/json-and-exit-codes.md#why-junit-xml), which nearly every test runner can write:

| Ecosystem | Command |
|---|---|
| Rust (nextest) | `cargo nextest run`, with `[profile.default.junit] path = "junit.xml"` in `.config/nextest.toml`; the report lands in `target/nextest/default/` |
| Rust (stable libtest) | `RUSTC_BOOTSTRAP=1 cargo test --tests -- -Z unstable-options --format junit > junit.xml` |
| Python | `pytest --junitxml=junit.xml` |
| Go | `go test -v ./... 2>&1 \| go-junit-report > junit.xml` |
| JS/TS | `vitest run --reporter=junit --outputFile=junit.xml`, or jest with `jest-junit` |
| JVM | Maven Surefire and Gradle write JUnit XML by default |

## Recording results

```bash
rqtk verify --results junit.xml
```

rqtk matches each test case to the functions linked to each activity and records one entry per activity in `.rqtk/evidence.toml`: the outcome, the tests, the commit, and the **content hash of the requirement at that moment**.

- An activity passes only when every linked test ran and passed; a skipped test leaves it incomplete.
- A run that covers only some tests, such as the Python suite without the Rust one, leaves other activities' evidence untouched.
- `verify` exits 1 if a linked test failed.
- `verify --check` writes nothing and exits 1 if the committed evidence doesn't match this run. Use it in CI.

## Coverage states

`rqtk coverage` puts every requirement in one state:

| State | Meaning | What to do |
|---|---|---|
| **Verified** | Every activity passed: linked tests against the current wording, or a manual status of Passed/Waived | nothing |
| **Suspect** | Tests passed, but for an earlier version of the requirement | run the tests, `rqtk verify` |
| **Failed** | A linked test failed, or a manual status is Failed | fix the code, or with the stakeholder, the requirement |
| **In Progress** | Some activities passed or started | finish the rest |
| **Planned** | Activities defined, nothing run yet | link and run tests |
| **Gap** | No activities, or no success criteria | define them |

`rqtk coverage --strict` exits 1 on any Gap, Failed or Suspect requirement, or unsatisfied need.

## Suspect

A requirement becomes Suspect when its **content hash** changes after its tests passed. The hash covers what the requirement demands and how it is verified: the statement, its structural links (parents, depends_on, derived_from, refines, satisfies), its parameters, and the verification method, level and phase. Editing the title, keywords, priority or notes changes nothing.

This is deliberate. A reworded requirement may no longer be what the tests check, so rqtk refuses to carry the old verdict over. `rqtk impact <base>` lists every activity to re-run after a change.

## Activities no test can check

Inspections, analyses and demonstrations keep a hand-written status:

```toml
[[verification.activities]]
id = "VA-SYS-002-01"
name = "Thermal analysis"
status = "Passed"
executed_at = 2026-03-14
evidence = ["reports/thermal-2026-03.pdf"]
```

As soon as a test is linked to an activity, its `status` is ignored, and `rqtk lint` warns until you remove it (RQ029).
