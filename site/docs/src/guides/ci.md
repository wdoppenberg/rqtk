# Continuous integration

Three commands make a complete requirements gate:

```bash
rqtk lint                                   # every file valid, every link resolves
rqtk verify --check --results junit.xml     # committed evidence matches this test run
rqtk coverage --strict                      # nothing Suspect, Failed or unverified
```

Each exits 1 when it finds a problem, so any CI system can use them directly.

## GitHub Actions

```yaml
name: Requirements

on: [push, pull_request]

jobs:
  requirements:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-nextest

      - name: Install rqtk
        run: curl -LsSf https://rqtk.dev/install.sh | sh

      - name: Test
        run: cargo nextest run --profile ci   # writes target/nextest/ci/junit.xml

      - run: rqtk lint
      - run: rqtk verify --check --results target/nextest/ci/junit.xml
      - run: rqtk coverage --strict
```

Configure the `ci` nextest profile to write JUnit in `.config/nextest.toml`:

```toml
[profile.ci.junit]
path = "junit.xml"
```

## Why `verify --check`

Evidence is committed, so it is reviewed like code. `verify --check` makes sure it is honest: it fails when the committed `.rqtk/evidence.toml` doesn't match what this CI run observed. For example, someone may have changed a requirement without re-running its tests, or a test may now fail. To fix it, run the tests and `rqtk verify` locally, then commit the updated evidence.

## Pull request review

`rqtk impact origin/main --json` lists the requirements a pull request changes, the requirements downstream of them, and the activities to re-verify. It is a good input for a review bot, or for the `/requirements-review` [agent skill](agents.md).
