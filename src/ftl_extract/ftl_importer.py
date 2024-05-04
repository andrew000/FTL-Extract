from __future__ import annotations

from pathlib import Path

from fluent.syntax import ast, parse

from ftl_extract.matcher import FluentKey


def import_from_ftl(path: Path, locale: str) -> tuple[dict[str, FluentKey], ast.Resource]:
    """Import `FluentKey`s from FTL."""
    ftl_keys = {}

    resource = parse(path.read_text(encoding="utf-8"), with_spans=True)

    for entry in resource.body:
        if isinstance(entry, ast.Message):
            ftl_keys[entry.id.name] = FluentKey(
                code_path=Path(),
                key=entry.id.name,
                translation=entry,
                # Cut off the locale from the path
                path=path,
                locale=locale,
            )

    return ftl_keys, resource


def import_ftl_from_dir(path: Path, locale: str) -> dict[str, FluentKey]:
    """Import `FluentKey`s from directory of FTL files."""
    ftl_files = (path / locale).rglob("*.ftl") if path.is_dir() else [path]
    ftl_keys = {}

    for ftl_file in ftl_files:
        keys, _ = import_from_ftl(ftl_file, locale)
        ftl_keys.update(keys)

    return ftl_keys
