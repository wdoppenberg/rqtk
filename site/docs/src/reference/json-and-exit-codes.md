# JSON output and exit codes

## `--json`

Every command accepts `--json` and prints exactly one JSON document on stdout.

- Paths are relative to the repository root.
- Diagnostics carry `code`, `severity`, `message`, `subject` (the item they concern) and `location` (`path`, `line`, `column`).
- Errors go to **stderr** as `{"error": {"kind": "usage" | "error", "message": "…"}}`, and stdout stays empty.
- Commands whose output is a document in another format (`report`, `graph`, `open`) reject `--json` with a usage error.

Within 1.x, JSON output only **gains** fields. No field is removed, renamed or given a different type; a test in the rqtk repository enforces this against a recorded snapshot of every command's output. Parse leniently and ignore fields you don't know.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success, nothing to report. |
| 1 | Findings: lint errors, Gap/Failed/Suspect requirements under `coverage --strict`, failed tests in `verify`, stale evidence under `verify --check`, link errors in `scan`. |
| 2 | Usage error: bad arguments, an unknown category, type, rule or schema kind, or `--json` on a command that doesn't support it. |
| 3 | Error: missing or invalid configuration, I/O or git failure. |

## Dry runs

Every command that writes files accepts `--dry-run`: `init`, `add`, `add-need`, `add-stakeholder`, `rehash`, `baseline`, `verify` and `skills install`. A dry run reports exactly what would be written and changes nothing.

## Why JUnit XML

`rqtk verify` needs one thing from a test run: which tests ran, and whether they passed, failed or were skipped. JUnit XML is the one results format that almost every test runner writes (nextest, pytest, go-junit-report, jest-junit, vitest, Maven, Gradle, .NET) and every CI system reads. Many projects already produce it for their CI's test reports.

Reading it keeps rqtk out of your test loop: rqtk records evidence and doesn't care how you run tests. The trade-off is that JUnit XML isn't strictly standardised, and reports rarely say which source file a test lives in. rqtk therefore matches test cases to linked functions by name, preferring results that mention the test's file.
