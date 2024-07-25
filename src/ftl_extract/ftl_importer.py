from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

from fluent.syntax import ast, parse

from ftl_extract.matcher import FluentKey

if TYPE_CHECKING:
    from fluent.syntax.ast import Resource


def import_from_ftl(
    path: Path, locale: str
) -> tuple[dict[str, FluentKey], Resource, list[FluentKey]]:
    """Import `FluentKey`s from FTL."""
    ftl_keys = {}
    leave_as_is = []

    resource = parse(path.read_text(encoding="utf-8"), with_spans=True)

    for position, entry in enumerate(resource.body, start=0):
        if isinstance(entry, ast.Message):
            ftl_keys[entry.id.name] = FluentKey(
                code_path=Path(),
                key=entry.id.name,
                translation=entry,
                path=path,
                locale=locale,
                position=position,
            )
        else:
            leave_as_is.append(
                FluentKey(
                    code_path=Path(),
                    key="",
                    translation=entry,
                    path=path,
                    locale=locale,
                    position=position,
                )
            )

    return ftl_keys, resource, leave_as_is


def import_ftl_from_dir(path: Path, locale: str) -> tuple[dict[str, FluentKey], list[FluentKey]]:
    """Import `FluentKey`s from directory of FTL files."""
    ftl_files = (path / locale).rglob("*.ftl") if path.is_dir() else [path]
    ftl_keys = {}
    leave_as_is = []

    for ftl_file in ftl_files:
        keys, _, as_is_keys = import_from_ftl(ftl_file, locale)
        ftl_keys.update(keys)
        leave_as_is.extend(as_is_keys)

    return ftl_keys, leave_as_is
