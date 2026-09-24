# Verifying with tests

A requirement is **verified** only by recorded test results, never by editing a file. rqtk records which tests passed against which version of each requirement. When the requirement changes afterwards, that evidence turns **Suspect** until the tests run again.

## The loop

1. **Link a test** to a verification activity.
2. **Run the tests** with JUnit XML output.
3. **Record the results** with `rqtk verify`.
4. **Check** with `rqtk coverage --strict`, and commit `.rqtk/evidence.toml` with the code.

## Linking tests

Put the activity ID directly above the test. Only blank lines, comments, other tags and attributes (`#[test]`, `@Test`, `@pytest.mark.parametrize(…)`, `[Fact]`) may sit in between; the first other line must be the test.

| Language | Annotation | Checked when |
|---|---|---|
| Rust | `#[rqtk::verifies("VA-…")]` (the `rqtk` crate with `default-features = false, features = ["macros"]`) | compile time |
| Python | `@rqtk.verifies("VA-…")` | test collection |
| Anything | `// rqtk: verifies VA-…` (or `#`, `--`, `/* */`) | `rqtk lint` |

rqtk recognises these test declarations:

| Language | Tests |
|---|---|
| Rust | `fn name` |
| Python | `def test_x`, `async def test_x`, methods of test classes |
| Go | `func TestX`, including methods `func (s *S) TestX` |
| JavaScript, TypeScript | `it`, `test` and `describe` (a whole group), with `.only`, `.skip`, `.concurrent`, `.each(…)`; `function name` |
| Java, C#, C, C++ | methods and functions (`void name()`, `public async Task Name()`), GoogleTest `TEST`, `TEST_F`, `TEST_P`, Catch2 and doctest `TEST_CASE` |
| Kotlin, Swift, Ruby, Zig, Elixir | `fun`, `func`, `def`, `test "name"`, RSpec `it "…" do` |

`rqtk scan` shows what each link is attached to. A tag with no test declaration below it can never match a result; `rqtk lint` reports it as an error (RQ030).

A test can verify several activities, and an activity can have several tests; it passes only when all of them pass. `rqtk lint` also reports links to unknown activities (RQ028).

### One case of a table-driven test

Tag the case's row, and name the case:

```go
func TestIntact(t *testing.T) {
    cases := []struct{ name string; flip bool }{
        // rqtk: verifies VA-SYS-0003-01 case "flipped byte is corrupt"
        {"flipped byte is corrupt", true},
        // rqtk: verifies VA-SYS-0003-02 case "intact file passes"
        {"intact file passes", false},
    }
    // …
}
```

The link belongs to the enclosing test and matches the runner's result for that case: Go's `TestIntact/flipped_byte_is_corrupt`, or pytest's `test_x[case]`. In Python and Rust, pass `case` to the annotation instead: `@rqtk.verifies("VA-…", case="neg")`.

### Choosing what is scanned

In `.rqtk/config.toml`:

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
| Go | `go test -v ./... 2>&1 \| go-junit-report > junit.xml` (`go install github.com/jstemmer/go-junit-report/v2@latest`), or `gotestsum --junitfile junit.xml` |
| JS/TS | `vitest run --reporter=junit --outputFile=junit.xml`, `bun test --reporter=junit --reporter-outfile=junit.xml`, or jest with `jest-junit` |
| JVM | Maven Surefire and Gradle write JUnit XML by default (`target/surefire-reports/*.xml`, `build/test-results/test/*.xml`) |
| C/C++ | `ctest --output-junit junit.xml` (CMake 3.21+), or GoogleTest's `--gtest_output=xml:junit.xml` |

`--results` takes several files, so one per package or module works too.

### How results are matched

A result matches a link by test name, together with the suite for GoogleTest and the case for a case link. When several tests share a name, rqtk narrows by the source file the runner reports (Bun, vitest, GoogleTest) or by what the link's path says about the result's class or module. If a linked test still matches several distinct tests, `rqtk verify` records nothing for it and says which tests it matched; rename one. Disabled and skipped tests leave an activity incomplete.

## Recording results

```bash
rqtk verify --results junit.xml
```

rqtk matches each test case to the functions linked to each activity and records one entry per activity in `.rqtk/evidence.toml`: the outcome, the tests, the commit, and the **content hash of the requirement at that moment**.

- An activity passes only when every linked test ran and passed; a skipped test leaves it incomplete.
- A run that covers only some tests, such as the Python suite without the Rust one, leaves other activities' evidence untouched.
- `verify` exits 1 if a linked test failed, if a linked test matched several tests, or if the results matched no linked test at all (usually the wrong file, or tags rqtk can't attach).
- `verify --check` writes nothing and exits 1 if the committed evidence doesn't match this run. Use it in CI.

## Coverage states

`rqtk coverage` puts every requirement in one state:

| State | Meaning | What to do |
|---|---|---|
| **Verified** | Every activity passed: linked tests against the current wording, or a manual status of Passed/Waived | nothing |
| **Suspect** | Evidence no longer settles it; see [Suspect](#suspect) | depends on the reason |
| **Failed** | A linked test failed, or a manual status is Failed | fix the code, or with the stakeholder, the requirement |
| **In Progress** | Some activities passed or started | finish the rest |
| **Planned** | Activities defined, nothing run yet | link and run tests |
| **Gap** | No activities, or no success criteria | define them |

`rqtk coverage --strict` exits 1 unless every requirement is Verified and every need is satisfied. For requirements written ahead of their implementation, `--allow planned` (and `--allow in-progress`) accepts those states too.

## Suspect

A requirement becomes Suspect when evidence recorded for it no longer settles it. `rqtk coverage` and `rqtk context` say why:

| Reason | What happened | What settles it |
|---|---|---|
| changed since its tests passed | its content hash changed after its tests passed | run the tests, `rqtk verify` |
| passed again with unchanged tests | it changed, and the same tests that passed for the old wording passed again | change the tests for the new wording and run them, or `rqtk review` |
| upstream changed | a parent requirement, or a need it or its ancestors satisfy, changed after it was verified | check it still fits, then `rqtk review` |

The **content hash** covers what the requirement demands and how it is verified: the statement, its structural links (parents, depends_on, derived_from, refines, satisfies), its parameters, and the verification method, level and phase. Editing the title, keywords, priority or notes changes nothing.

This is deliberate. A reworded requirement may no longer be what the tests check, so rqtk refuses to carry the old verdict over, and rerunning a test that was never updated proves nothing new. `rqtk impact <base>` lists every activity to re-run after a change. [Change control](change-control.md#review-debt) covers `rqtk review`.

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
