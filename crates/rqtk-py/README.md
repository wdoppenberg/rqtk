# rqtk

> **rqtk**: requirements toolkit

Requirements that live in your repository, proven by tests. Documentation: **[rqtk.dev](https://rqtk.dev)**.

```bash
pip install rqtk        # or: uv tool install rqtk
rqtk init
rqtk lint
```

This package contains the full `rqtk` command line and the `verifies` decorator, which links a Python test to a verification activity:

```python
import rqtk

@rqtk.verifies("VA-SYS-001-01")
def test_boots_in_under_five_seconds():
    ...
```

An unknown activity ID raises `ValueError` at import time, so test collection fails before any test runs. Record test results with `pytest --junitxml=junit.xml` and `rqtk verify --results junit.xml`. See [Verifying with tests](https://rqtk.dev/docs/guides/verification.html).
