from __future__ import annotations

from pathlib import Path

from fluent.syntax import ast, parse

from ftl_extract.matcher import FluentKey


def import_from_ftl(
    path: Path, locale: str
) -> tuple[
    dict[str, FluentKey],
    ast.Resource,
    list[ast.Term | ast.Comment | ast.GroupComment | ast.ResourceComment | ast.Junk],
]:
    """Import `FluentKey`s from FTL."""
    ftl_keys = {}
    leave_as_is = []

    resource = parse(path.read_text(encoding="utf-8"), with_spans=True)

    for entry in resource.body:
        if isinstance(entry, ast.Message):
            ftl_keys[entry.id.name] = FluentKey(
                code_path=Path(),
                key=entry.id.name,
                translation=entry,
                path=path,
                locale=locale,
            )
        else:
            leave_as_is.append(entry)

    return ftl_keys, resource, leave_as_is


def import_ftl_from_dir(
    path: Path, locale: str
) -> tuple[
    dict[str, FluentKey],
    list[ast.Term | ast.Comment | ast.GroupComment | ast.ResourceComment | ast.Junk],
]:
    """Import `FluentKey`s from directory of FTL files."""
    ftl_files = (path / locale).rglob("*.ftl") if path.is_dir() else [path]
    ftl_keys = {}
    leave_as_is = []

    for ftl_file in ftl_files:
        keys, _, as_is_keys = import_from_ftl(ftl_file, locale)
        ftl_keys.update(keys)
        leave_as_is.extend(as_is_keys)

    return ftl_keys, leave_as_is
