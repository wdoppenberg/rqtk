# Client conformance suite

Every client (Rust, Python and TypeScript) opens the same repositories, makes the same calls and must return what `expected/` holds. This is RQTK-CLIENT-0017: the three bindings share one engine, and the suite catches drift in the glue between it and each language.

- `firesat.bundle`: the git repository most cases open. `scripts/build-conformance-fixture.sh` builds it from `crates/rqtk/tests/fixtures/firesat-obc` with fixed names and dates, so its commits are the same on every build.
- `cases.json`: the fixtures (the bundle, a copy without git, small broken repositories) and the cases. A case says how to open (`fixture` with an optional `ref`, `remote` for a fixture by `file://` URL, or `url`), which method to call (by its Rust and Python name; TypeScript camel-cases it) and its arguments.
- `expected/<case>.json`: `{"result": …}` or, when opening or the call fails, `{"error": "<code>"}`. `$ROOT` and `$URL` stand for where the run put the opened fixture.

The runners are `crates/rqtk-client/tests/conformance.rs`, `crates/rqtk-py/tests/test_conformance.py` and `crates/rqtk-node/tests/conformance.test.mjs`. Cases named `error-…` cover every error code (RQTK-CLIENT-0009); `find_requirements` cases cover every filter field (RQTK-CLIENT-0011).

After changing the fixture or the client's output, regenerate and review the diff:

```sh
scripts/build-conformance-fixture.sh
RQTK_BLESS=1 cargo test -p rqtk-client --test conformance
git diff conformance/
```

Blessing never deletes the file of a removed case; the next run names it.
