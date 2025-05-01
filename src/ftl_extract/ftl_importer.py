from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

from fluent.syntax import FluentParser, ast

from ftl_extract.matcher import FluentKey

if TYPE_CHECKING:
    from fluent.syntax.ast import Resource

    from ftl_extract.utils import ExtractionStatistics


def import_from_ftl(
    *,
    path: Path,
    locale: str,
    parser: FluentParser,
) -> tuple[dict[str, FluentKey], dict[str, FluentKey], Resource, list[FluentKey]]:
    """Import `FluentKey`s from FTL."""
    ftl_keys: dict[str, FluentKey] = {}
    terms: dict[str, FluentKey] = {}
    leave_as_is = []

    resource = parser.parse(path.read_text(encoding="utf-8"))

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
        elif isinstance(entry, ast.Term):
            terms[entry.id.name] = FluentKey(
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
                ),
            )

    return ftl_keys, terms, resource, leave_as_is


def import_ftl_from_dir(
    *,
    path: Path,
    locale: str,
    statistics: ExtractionStatistics | None = None,
) -> tuple[dict[str, FluentKey], dict[str, FluentKey], list[FluentKey]]:
    """Import `FluentKey`s from directory of FTL files."""
    ftl_files = (path / locale).rglob("*.ftl") if path.is_dir() else [path]
    stored_ftl_keys: dict[str, FluentKey] = {}
    stored_terms: dict[str, FluentKey] = {}
    stored_leave_as_is_keys = []
    parser = FluentParser(with_spans=True)

    for ftl_file in ftl_files:
        keys, terms, _, leave_as_is_keys = import_from_ftl(
            path=ftl_file,
            locale=locale,
            parser=parser,
        )
        stored_ftl_keys.update(keys)
        stored_terms.update(terms)
        stored_leave_as_is_keys.extend(leave_as_is_keys)

        if statistics:
            statistics.ftl_files_count[locale] += 1

    return stored_ftl_keys, stored_terms, stored_leave_as_is_keys
