"""rqtk: requirements toolkit.

Requirements that live in your repository, proven by tests. See https://rqtk.dev.
"""

from importlib.metadata import version as _version

from rqtk._rqtk import verifies

__all__ = ["verifies"]
__version__ = _version("rqtk")
