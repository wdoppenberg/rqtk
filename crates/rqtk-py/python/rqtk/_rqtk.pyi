from typing import Callable, TypeVar

_F = TypeVar("_F", bound=Callable[..., object])

class verifies:
    """Link a test to a verification activity. Raises ValueError for an unknown activity ID.

    ``case`` links one case of a parametrized test, by its pytest id (``test_x[case]``).
    """

    def __init__(self, activity_id: str, *, case: str | None = None) -> None: ...
    def __call__(self, wraps: _F) -> _F: ...

def run_cli(argv: list[str]) -> int:
    """Run the rqtk command line with argv (argv[0] is the program name); return the exit code."""
