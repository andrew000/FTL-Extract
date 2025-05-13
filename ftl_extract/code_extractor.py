from __future__ import annotations

import ast
from pathlib import Path
from typing import TYPE_CHECKING, cast

from fluent.syntax import ast as fluent_ast

from ftl_extract.exceptions import (
    FTLExtractorDifferentPathsError,
    FTLExtractorDifferentTranslationError,
)
from ftl_extract.matcher import FluentKey, I18nMatcher

if TYPE_CHECKING:
    from collections.abc import Iterable, Iterator

    from ftl_extract.matcher import FluentKey
    from ftl_extract.utils import ExtractionStatistics


def find_py_files(*, search_path: Path, exclude_dirs: frozenset[Path]) -> Iterator[Path]:
    """
    First step: find all .py files in given path.

    :param search_path: Path to directory with .py files.
    :type search_path: Path
    :param exclude_dirs: Exclude directories from search.
    :type exclude_dirs: frozenset[Path]
    :return: Iterator with Path to .py files.
    :rtype: Iterator[Path]
    """
    if search_path.is_dir():
        for path in search_path.rglob("[!{.}]*.py"):
            if path.is_file() and not any(
                path.is_relative_to(exclude_dir) for exclude_dir in exclude_dirs
            ):
                yield path
    else:
        # If search_path is not a directory, check if it is a file
        if search_path.is_file() and search_path.suffix == ".py":
            yield search_path
        else:
            # If search_path is not a directory or file, return an empty iterator
            yield from iter([])


def parse_file(
    *,
    path: Path,
    i18n_keys: Iterable[str],
    i18n_keys_prefix: Iterable[str],
    ignore_attributes: Iterable[str],
    ignore_kwargs: Iterable[str],
    default_ftl_file: Path,
) -> dict[str, FluentKey]:
    """
    Second step: parse given .py file and find all i18n calls.

    :param path: Path to .py file.
    :type path: Path
    :param i18n_keys: Names of function that is used to get translation.
    :type i18n_keys: Iterable[str]
    :param i18n_keys_prefix: Prefix names of function that is used to get translation.
    :type i18n_keys_prefix: Iterable[str]
    :param ignore_attributes: Ignore attributes, like `i18n.set_locale`.
    :type ignore_attributes: Iterable[str]
    :param ignore_kwargs: Ignore kwargs, like `when` from
    `aiogram_dialog.I18nFormat(..., when=...)`.
    :type ignore_kwargs: Iterable[str]
    :param default_ftl_file: Default name of FTL file.
    :type default_ftl_file: Path
    :return: Dict with `key` and `FluentKey`.
    :rtype: dict[str, FluentKey]
    """
    node = ast.parse(path.read_bytes())
    matcher = I18nMatcher(
        code_path=path,
        default_ftl_file=default_ftl_file,
        i18n_keys=i18n_keys,
        i18n_keys_prefix=i18n_keys_prefix,
        ignore_attributes=ignore_attributes,
        ignore_kwargs=ignore_kwargs,
    )
    matcher.visit(node)
    return matcher.fluent_keys


def post_process_fluent_keys(*, fluent_keys: dict[str, FluentKey], default_ftl_file: Path) -> None:
    """
    Third step: post-process parsed `FluentKey`.

    :param fluent_keys: Dict with `key` and `FluentKey` that will be post-processed.
    :type fluent_keys: dict[str, FluentKey]
    :param default_ftl_file: Default name of FTL file.
    :type default_ftl_file: Path
    """
    for fluent_key in fluent_keys.values():
        if not isinstance(fluent_key.path, Path):
            fluent_key.path = Path(fluent_key.path)

        if not fluent_key.path.suffix:  # if path looks like directory (no suffix)
            fluent_key.path /= default_ftl_file


def find_conflicts(
    *,
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
    *,
    path: Path,
    i18n_keys: Iterable[str],
    i18n_keys_prefix: Iterable[str],
    exclude_dirs: frozenset[Path],
    ignore_attributes: Iterable[str],
    ignore_kwargs: Iterable[str],
    default_ftl_file: Path,
    statistics: ExtractionStatistics | None = None,
) -> dict[str, FluentKey]:
    """
    Extract all `FluentKey`s from given path.

    :param path: Path to [.py file] / [directory with .py files].
    :type path: Path
    :param i18n_keys: Names of function that is used to get translation.
    :type i18n_keys: Iterable[str]
    :param i18n_keys_prefix: Prefix names of function that is used to get translation.
    :type i18n_keys_prefix: Iterable[str]
    :param exclude_dirs: Exclude directories from search.
    :type exclude_dirs: frozenset[Path]
    :param ignore_attributes: Ignore attributes, like `i18n.set_locale`.
    :type ignore_attributes: Iterable[str]
    :param ignore_kwargs: Ignore kwargs, like `when` from
    `aiogram_dialog.I18nFormat(..., when=...)`.
    :type ignore_kwargs: Iterable[str]
    :param default_ftl_file: Default name of FTL file.
    :type default_ftl_file: Path
    :param statistics: Statistics of extraction.
    :type statistics: ExtractionStatistics
    :return: Dict with `key` and `FluentKey`.
    :rtype: dict[str, FluentKey]

    """
    fluent_keys: dict[str, FluentKey] = {}

    for file in find_py_files(search_path=path, exclude_dirs=exclude_dirs):
        keys = parse_file(
            path=file,
            i18n_keys=i18n_keys,
            i18n_keys_prefix=i18n_keys_prefix,
            ignore_attributes=ignore_attributes,
            ignore_kwargs=ignore_kwargs,
            default_ftl_file=default_ftl_file,
        )
        post_process_fluent_keys(fluent_keys=keys, default_ftl_file=default_ftl_file)
        find_conflicts(current_fluent_keys=fluent_keys, new_fluent_keys=keys)
        fluent_keys.update(keys)

        if statistics and keys:
            statistics.py_files_count += 1

    return fluent_keys


def sort_fluent_keys_by_path(*, fluent_keys: dict[str, FluentKey]) -> dict[Path, list[FluentKey]]:
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
