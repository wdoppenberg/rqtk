"""Reading rqtk repositories from Python: `rqtk.open`, `rqtk.open_remote` and `Repository`.

The Rust client does the work, without the GIL; this module turns its JSON into the frozen
dataclasses of `rqtk.types`.
"""

from __future__ import annotations

import dataclasses
import json
import os
from pathlib import Path
from typing import Any, Callable, List, Mapping, Optional, TypeVar, Union

from rqtk import _rqtk
from rqtk import types as t

_T = TypeVar("_T")


class Error(Exception):
    """A client error. Branch on `code`, one of a fixed list (`rqtk.types.ErrorCode`), not on
    the message."""

    code: t.ErrorCode
    message: str

    def __init__(self, code: t.ErrorCode, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message

    def __repr__(self) -> str:
        return f"rqtk.Error({self.code!r}, {self.message!r})"


def _decode(decoder: Callable[[Any], _T], text: str) -> _T:
    return decoder(json.loads(text))


def _decode_all(decoder: Callable[[Any], _T], text: str) -> List[_T]:
    return [decoder(item) for item in json.loads(text)]


def _filter_json(filter: Union[t.Filter, Mapping[str, Any], None]) -> str:
    if filter is None:
        return "{}"
    if isinstance(filter, t.Filter):
        filter = dataclasses.asdict(filter)
    fields = {k: list(v) if isinstance(v, (tuple, list)) else v for k, v in filter.items()}
    return json.dumps({k: v for k, v in fields.items() if v is not None})


class Repository:
    """An rqtk repository as it was when it opened: requirements, needs, stakeholders, their
    links, verification status and lint findings.

    A snapshot: later edits are not seen until `reload()`, which returns a new one. Methods
    release the GIL; one repository answers one query at a time, separate ones in parallel.
    """

    __slots__ = ("_native",)

    def __init__(self, native: _rqtk.Repository) -> None:
        self._native = native

    def __repr__(self) -> str:
        return f"<rqtk.Repository {self.info().origin!r}>"

    def reload(self) -> Repository:
        """Open the same source again: the working tree as it is now, or the ref's or URL's
        current commit."""
        return Repository(self._native.reload())

    def info(self) -> t.RepositoryInfo:
        """Where the snapshot came from and its commit."""
        return _decode(t._decode_RepositoryInfo, self._native.info())

    # Items -----------------------------------------------------------------------------------

    def requirements(self) -> List[t.RequirementEntry]:
        """Every requirement, in ID order, with its status and links in both directions."""
        return _decode_all(t._decode_RequirementEntry, self._native.requirements())

    def requirement(self, id: str) -> Optional[t.RequirementEntry]:
        """The requirement `id`, or None."""
        found = self._native.requirement(id)
        return None if found is None else _decode(t._decode_RequirementEntry, found)

    def find_requirements(
        self, filter: Union[t.Filter, Mapping[str, Any], None] = None
    ) -> List[t.RequirementEntry]:
        """The requirements matching `filter`: a `Filter`, or a dict with its fields."""
        return _decode_all(
            t._decode_RequirementEntry, self._native.find_requirements(_filter_json(filter))
        )

    def needs(self) -> List[t.NeedEntry]:
        """Every need, in ID order, with its links and whether a requirement satisfies it."""
        return _decode_all(t._decode_NeedEntry, self._native.needs())

    def need(self, id: str) -> Optional[t.NeedEntry]:
        """The need `id`, or None."""
        found = self._native.need(id)
        return None if found is None else _decode(t._decode_NeedEntry, found)

    def stakeholders(self) -> List[t.StakeholderEntry]:
        """Every stakeholder, in ID order, with the needs they raised."""
        return _decode_all(t._decode_StakeholderEntry, self._native.stakeholders())

    def stakeholder(self, id: str) -> Optional[t.StakeholderEntry]:
        """The stakeholder `id`, or None."""
        found = self._native.stakeholder(id)
        return None if found is None else _decode(t._decode_StakeholderEntry, found)

    def activities(self) -> List[t.ActivityEntry]:
        """Every verification activity, with its state, linked tests and evidence."""
        return _decode_all(t._decode_ActivityEntry, self._native.activities())

    # Views: the documents `rqtk <command> --json` prints --------------------------------------

    def context(self, id: str) -> t.ContextDoc:
        """Everything relevant to one item (`rqtk context`). Raises `Error` (`not_found`)."""
        return _decode(t._decode_ContextDoc, self._native.context(id))

    def trace(self, id: str) -> t.TraceDoc:
        """A requirement's ancestors and descendants (`rqtk trace`)."""
        return _decode(t._decode_TraceDoc, self._native.trace(id))

    def coverage(self) -> t.CoverageDoc:
        """Verification status of every requirement and need (`rqtk coverage`)."""
        return _decode(t._decode_CoverageDoc, self._native.coverage())

    def lint(self) -> t.LintDoc:
        """Lint findings (`rqtk lint`)."""
        return _decode(t._decode_LintDoc, self._native.lint())

    def impact(self, base: str) -> t.ImpactDoc:
        """What changed since revision `base` and what to re-verify (`rqtk impact`)."""
        return _decode(t._decode_ImpactDoc, self._native.impact(base))

    def diff(self, from_ref: str, to_ref: str) -> t.DiffDoc:
        """Requirements added, removed and modified between two revisions (`rqtk diff`)."""
        return _decode(t._decode_DiffDoc, self._native.diff(from_ref, to_ref))

    def baselines(self) -> t.BaselinesDoc:
        """The repository's baselines (`rqtk baseline --list`)."""
        return _decode(t._decode_BaselinesDoc, self._native.baselines())

    def history(self, id: str) -> t.HistoryDoc:
        """The commits that changed a requirement, newest first (`rqtk log`)."""
        return _decode(t._decode_HistoryDoc, self._native.history(id))


def open(
    path: Union[str, os.PathLike[str]] = ".", *, ref: Optional[str] = None
) -> Repository:
    """Open the repository whose root (the directory holding `.rqtk/`) is `path`.

    With `ref` (a tag, baseline name, branch or commit), read it at that revision from git
    instead of the working tree. Raises `Error` only when the repository can't be read.
    """
    return Repository(_rqtk.open(os.fspath(path), ref))


def open_remote(url: str, ref: str, *, allow_file_url: bool = False) -> Repository:
    """Open the repository at git URL `url` (https or SSH, with git's own authentication), at
    revision `ref`. Fetched into the cache (`cache_dir()`), so later opens fetch only what's
    new. `allow_file_url` also accepts `file://` URLs and local paths."""
    return Repository(_rqtk.open_remote(url, ref, allow_file_url))


def cache_dir() -> Path:
    """Where `open_remote` keeps fetched repositories. `RQTK_CACHE_DIR` overrides it."""
    return Path(_rqtk.cache_dir())


def clear_cache() -> None:
    """Delete every cached repository. Nothing else ever deletes them."""
    _rqtk.clear_cache()
