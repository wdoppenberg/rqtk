"""rqtk: requirements toolkit.

Requirements that live in your repository, proven by tests. See https://rqtk.dev.

- `rqtk.open(path)` and `rqtk.open_remote(url, ref)` read a repository; the data types are in
  `rqtk.types`.
- `@rqtk.verifies("VA-…")` links a test to a verification activity.
"""

from importlib.metadata import version as _version

from rqtk import types
from rqtk._client import Error, Repository, cache_dir, clear_cache, open, open_remote
from rqtk._rqtk import verifies
from rqtk.types import Filter

__all__ = [
    "Error",
    "Filter",
    "Repository",
    "cache_dir",
    "clear_cache",
    "open",
    "open_remote",
    "types",
    "verifies",
]
__version__ = _version("rqtk")
