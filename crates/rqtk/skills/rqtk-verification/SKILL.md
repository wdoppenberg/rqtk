---
name: rqtk-verification
description: Prove requirements with tests. Link tests to verification activities with `verifies`, record test results with `rqtk verify`, and close out with `rqtk coverage --strict`. Use when implementing or changing behaviour a requirement governs, when a requirement is Suspect, Failed or Planned, or when the user mentions verification, evidence, coverage or an activity ID such as VA-SYS-001-01.
compatibility: Requires the rqtk CLI and a repository set up with rqtk init.
---

# Verifying requirements

A requirement is **verified** only by recorded test results, never by editing a file. rqtk records which tests passed against which version of each requirement in `.rqtk/evidence.toml`. When the requirement changes afterwards, that evidence turns **Suspect** until the tests run again.

## The loop

1. **Pick the activity.** `rqtk context <ID> --json` lists the requirement's activities, the state of each, and the tests already linked to them.
2. **Link a test.** Put the activity ID directly above the test function:
   - Rust: `#[rqtk_macros::verifies("VA-…")]`. An unknown ID is a compile error.
   - Python: `@rqtk.verifies("VA-…")`
   - Any other language: a comment `// rqtk: verifies VA-…` (or `#`, `--`) on the line above the test.

   The test checks what the statement demands, through the public interface. When a `tdd` skill is available, call the Skill tool with "tdd" for the red → green loop; this skill adds the traceability on top.
3. **Record the results.** Run the tests with JUnit output, then `rqtk verify --results <junit.xml>`. An activity passes only when every test linked to it ran and passed. A partial run leaves other activities' evidence untouched.
4. **Close out.** Done means `rqtk lint` and `rqtk coverage --strict` both exit 0. Commit `.rqtk/evidence.toml` together with the code.

## JUnit output per ecosystem

| Ecosystem | Command |
|---|---|
| Rust (nextest) | `cargo nextest run`, with `[profile.default.junit] path = "junit.xml"` in `.config/nextest.toml`; the file lands in `target/nextest/default/` |
| Rust (stable libtest) | `RUSTC_BOOTSTRAP=1 cargo test --tests -- -Z unstable-options --format junit > junit.xml` |
| Python | `pytest --junitxml=junit.xml` |
| Go | `go test -v ./... 2>&1 \| go-junit-report > junit.xml` |
| JS/TS | `vitest run --reporter=junit --outputFile=junit.xml`, or jest with `jest-junit` |

## Reading coverage

| State | Meaning | Action |
|---|---|---|
| Verified | Linked tests passed against the current statement | none |
| Suspect | Tests passed for an earlier version of the requirement | rerun its tests, `rqtk verify` |
| Failed | A linked test failed | fix the code (or, with the user, the requirement) |
| Planned / not run | Activities exist, no evidence yet | link or run the tests |
| Gap | No activities or no success criteria | add them (see the `rqtk-requirements` skill) |

Activities that no test can check (inspection, analysis, demonstration) keep a hand-written `status`. For every other activity, `status` stays unset.

After changing requirements or tests, `rqtk impact <base>` lists every activity to re-run and every downstream requirement to review.
