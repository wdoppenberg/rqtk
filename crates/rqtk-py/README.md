# rqtk

> **rqtk**: requirements toolkit

Requirements that live in your repository, proven by tests. Documentation: **[rqtk.dev](https://rqtk.dev)**.

```bash
pip install rqtk        # or: uv tool install rqtk
rqtk init
rqtk lint
```

This package contains the full `rqtk` command line, a client for reading repositories from Python, and the `verifies` decorator.

## Reading a repository

```python
import rqtk

repo = rqtk.open(".")                       # or rqtk.open(".", ref="1.4.0"), rqtk.open_remote(url, ref)
for entry in repo.find_requirements(rqtk.Filter(status=("suspect", "failed"))):
    print(entry.id, entry.status, entry.requirement.title)
print(repo.coverage().summary)
```

Values are frozen dataclasses (`rqtk.types`) with the same field names as `rqtk <command> --json`, so `dataclasses.asdict` gives JSON-ready data. See [Reading repositories from code](https://rqtk.dev/docs/guides/client.html).

## Linking tests

The `verifies` decorator links a Python test to a verification activity:

```python
import rqtk

@rqtk.verifies("VA-SYS-001-01")
def test_boots_in_under_five_seconds():
    ...
```

An unknown activity ID raises `ValueError` at import time, so test collection fails before any test runs. Record test results with `pytest --junitxml=junit.xml` and `rqtk verify --results junit.xml`. See [Verifying with tests](https://rqtk.dev/docs/guides/verification.html).
