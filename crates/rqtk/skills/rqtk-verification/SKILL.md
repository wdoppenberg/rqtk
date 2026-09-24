---
name: rqtk-verification
description: Prove requirements with tests. Link tests to verification activities with `verifies`, record test results with `rqtk verify`, and close out with `rqtk coverage --strict`. Use when implementing or changing behaviour a requirement governs, when a requirement is Suspect, Failed or Planned, or when the user mentions verification, evidence, coverage or an activity ID such as VA-SYS-0001-01.
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI and a repository set up with rqtk init.
---

# Verifying requirements

A requirement is **verified** only by recorded test results, never by editing a file. rqtk records which tests passed against which version of each requirement in `.rqtk/evidence.toml`. When the requirement, or anything it derives from, changes afterwards, it turns **Suspect**.

## The loop

1. **Pick the activity.** `rqtk context <ID> --json` lists the requirement's activities, the state of each, and the tests already linked to them.
2. **Link a test.** Put the activity ID directly above the test; only comments and attributes may sit in between:
   - Rust: `#[rqtk::verifies("VA-…")]`, from the `rqtk` crate as a dev-dependency with `default-features = false, features = ["macros"]`. An unknown ID is a compile error.
   - Python: `@rqtk.verifies("VA-…")`
   - Any other language: a comment `// rqtk: verifies VA-…` (or `#`, `--`) above the test.
   - One case of a table-driven or parameterised test: tag its row with `// rqtk: verifies VA-… case "case name"`, or pass `case="…"` to `verifies`.

   Run `rqtk scan` and check the link shows the test's name, not "(no test declaration)". The test checks what the statement demands, through the public interface. When a `tdd` skill is available, call the Skill tool with "tdd" for the red → green loop; this skill adds the traceability on top.
3. **Record the results.** Run the tests with JUnit output, then `rqtk verify --results <junit.xml>`. An activity passes only when every test linked to it ran and passed. A partial run leaves other activities' evidence untouched. `verify` exits 1 on a failing test, on a linked test name that matches several tests (rename one), and when no result matched any link.
4. **Close out.** Done means `rqtk lint` and `rqtk coverage --strict` both exit 0: every requirement Verified. Commit `.rqtk/evidence.toml` together with the code.

## JUnit output per ecosystem

| Ecosystem | Command |
|---|---|
| Rust (nextest) | `cargo nextest run`, with `[profile.default.junit] path = "junit.xml"` in `.config/nextest.toml`; the file lands in `target/nextest/default/` |
| Rust (stable libtest) | `RUSTC_BOOTSTRAP=1 cargo test --tests -- -Z unstable-options --format junit > junit.xml` |
| Python | `pytest --junitxml=junit.xml` |
| Go | `go test -v ./... 2>&1 \| go-junit-report > junit.xml` (`go install github.com/jstemmer/go-junit-report/v2@latest`), or `gotestsum --junitfile junit.xml` |
| JS/TS | `vitest run --reporter=junit --outputFile=junit.xml`, `bun test --reporter=junit --reporter-outfile=junit.xml`, or jest with `jest-junit` |
| JVM | Maven Surefire and Gradle write JUnit XML by default (`target/surefire-reports/*.xml`, `build/test-results/test/*.xml`) |
| C/C++ | `ctest --output-junit junit.xml` (CMake 3.21+), or GoogleTest's `--gtest_output=xml:junit.xml` |

## Reading coverage

| State | Meaning | Action |
|---|---|---|
| Verified | Linked tests passed against the current statement | none |
| Suspect | Evidence no longer settles it; `rqtk coverage` says why | see below |
| Failed | A linked test failed | fix the code (or, with the user, the requirement) |
| In Progress / Planned | Activities exist, not all have evidence | link or run the tests |
| Gap | No activities or no success criteria | add them (see the `rqtk-requirements` skill) |

Suspect has three reasons:

- **changed since its tests passed**: rerun its tests and `rqtk verify`.
- **passed again with unchanged tests**: the same tests that passed for the old wording passed again, which proves nothing about the new one. Update the tests for the new wording and rerun them. If they already check it, tell the user; confirming that is theirs, with `rqtk review <ID> --note "…"`.
- **upstream changed**: a parent requirement or need changed. Check the requirement still fits it (and change it if not); the user confirms with `rqtk review <ID>`.

Activities that no test can check (inspection, analysis, demonstration) keep a hand-written `status`. For every other activity, `status` stays unset.

After changing requirements or tests, `rqtk impact <base>` lists every activity to re-run with its tests, and every downstream requirement to review.
