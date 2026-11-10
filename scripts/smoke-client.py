"""Opens a fixture with an installed `rqtk` wheel: the check that a prebuilt wheel works on the
platform it was built for.

    python smoke-client.py <repository>
"""

import platform
import sys

import rqtk

repo = rqtk.open(sys.argv[1])
ids = [r.id for r in repo.requirements()]
if "FOBC-SYS-0001" not in ids or len(repo.coverage().requirements) != len(ids):
    raise SystemExit(f"unexpected requirements: {ids}")
print(f"rqtk on {sys.platform}-{platform.machine()}: {len(ids)} requirements")
