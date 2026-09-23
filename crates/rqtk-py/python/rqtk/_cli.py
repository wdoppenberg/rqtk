"""The `rqtk` console command: runs the command line in-process."""

import sys

from rqtk._rqtk import run_cli


def main() -> None:
    sys.exit(run_cli(["rqtk", *sys.argv[1:]]))
