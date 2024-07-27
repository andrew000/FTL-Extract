from __future__ import annotations

import ast
from pathlib import Path
from typing import TYPE_CHECKING, cast

from fluent.syntax import ast as fluent_ast

from ftl_extract.exceptions import (
    FTLExtractorDifferentPathsError,
    FTLExtractorDifferentTranslationError,
)
from ftl_extract.matcher import I18nMatcher

if TYPE_CHECKING:
    from collections.abc import Iterable, Iterator

    from ftl_extract.matcher import FluentKey


def find_py_files(path: Path) -> Iterator[Path]:
    """
    First step: find all .py files in given path.

    :param path: Path to directory with .py files.
    :type path: Path
    :return: Iterator with Path to .py files.
    :rtype: Iterator[Path]
    """
    yield from path.rglob("[!{.}]*.py") if path.is_dir() else [path]


def parse_file(
    path: Path,
    i18n_keys: str | Iterable[str],
    ignore_attributes: Iterable[str],
) -> dict[str, FluentKey]:
    """
    Second step: parse given .py file and find all i18n calls.

    :param path: Path to .py file.
    :type path: Path
    :param i18n_keys: Names of function that is used to get translation.
    :type i18n_keys: str | Sequence[str]
    :param ignore_attributes: Ignore attributes, like `i18n.set_locale`.
    :type ignore_attributes: Sequence[str]
    :return: Dict with `key` and `FluentKey`.
    :rtype: dict[str, FluentKey]
    """
    node = ast.parse(path.read_bytes())
    matcher = I18nMatcher(code_path=path, func_names=i18n_keys, ignore_attributes=ignore_attributes)
    matcher.visit(node)
    return matcher.fluent_keys


def post_process_fluent_keys(fluent_keys: dict[str, FluentKey]) -> None:
    """
    Third step: post-process parsed `FluentKey`.

    :param fluent_keys: Dict with `key` and `FluentKey` that will be post-processed.
    :type fluent_keys: dict[str, FluentKey]
    """
    for fluent_key in fluent_keys.values():
        if not isinstance(fluent_key.path, Path):
            fluent_key.path = Path(fluent_key.path)

        if not fluent_key.path.suffix:  # if path looks like directory (no suffix)
            fluent_key.path /= "_default.ftl"


def find_conflicts(
    current_fluent_keys: dict[str, FluentKey],
    new_fluent_keys: dict[str, FluentKey],
) -> None:
    """
    Fourth step: find conflicts between current and new `FluentKey`s.

    If conflict is found, raise `ValueError`.

    Conflict is when `key` is the same, but `path` or `kwargs` are different.
    """
    # Find common keys
    conflict_keys = set(current_fluent_keys.keys()) & set(new_fluent_keys.keys())

    if not conflict_keys:
        return

    for key in conflict_keys:
        if current_fluent_keys[key].path != new_fluent_keys[key].path:
            raise FTLExtractorDifferentPathsError(
                key,
                current_fluent_keys[key].path,
                new_fluent_keys[key].path,
            )

        if not current_fluent_keys[key].translation.equals(new_fluent_keys[key].translation):
            raise FTLExtractorDifferentTranslationError(
                key,
                cast(fluent_ast.Message, current_fluent_keys[key].translation),
                cast(fluent_ast.Message, new_fluent_keys[key].translation),
            )


def extract_fluent_keys(
    path: Path,
    i18n_keys: str | Iterable[str],
    ignore_attributes: Iterable[str],
) -> dict[str, FluentKey]:
    """
    Extract all `FluentKey`s from given path.

    :param path: Path to [.py file] / [directory with .py files].
    :type path: Path
    :param i18n_keys: Names of function that is used to get translation.
    :type i18n_keys: str | Sequence[str]
    :param ignore_attributes: Ignore attributes, like `i18n.set_locale`.
    :type ignore_attributes: Sequence[str]
    :return: Dict with `key` and `FluentKey`.
    :rtype: dict[str, FluentKey]

    """
    fluent_keys: dict[str, FluentKey] = {}

    for file in find_py_files(path):
        keys = parse_file(path=file, i18n_keys=i18n_keys, ignore_attributes=ignore_attributes)
        post_process_fluent_keys(keys)
        find_conflicts(fluent_keys, keys)
        fluent_keys.update(keys)

    return fluent_keys


def sort_fluent_keys_by_path(fluent_keys: dict[str, FluentKey]) -> dict[Path, list[FluentKey]]:
    """
    Sort `FluentKey`s by their paths.

    :param fluent_keys: Dict with `key` and `FluentKey`.
    :type fluent_keys: dict[str, FluentKey]
    :return: Dict with `Path` and list of `FluentKey`.
    :rtype: dict[Path, list[FluentKey]]
    """
    sorted_fluent_keys: dict[Path, list[FluentKey]] = {}
    for fluent_key in fluent_keys.values():
        sorted_fluent_keys.setdefault(fluent_key.path, []).append(fluent_key)

    return sorted_fluent_keys
