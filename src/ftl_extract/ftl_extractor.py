from __future__ import annotations

from typing import TYPE_CHECKING

from click import echo
from fluent.syntax import FluentSerializer
from fluent.syntax import ast as fl_ast

from ftl_extract import extract_fluent_keys
from ftl_extract.code_extractor import sort_fluent_keys_by_path
from ftl_extract.const import (
    COMMENT_KEYS_MODE,
    DEFAULT_EXCLUDE_DIRS,
    DEFAULT_FTL_FILE,
    DEFAULT_I18N_KEYS,
    DEFAULT_IGNORE_ATTRIBUTES,
    DEFAULT_IGNORE_KWARGS,
)
from ftl_extract.ftl_importer import import_ftl_from_dir
from ftl_extract.process.commentator import comment_ftl_key
from ftl_extract.process.kwargs_extractor import extract_kwargs
from ftl_extract.process.serializer import generate_ftl
from ftl_extract.utils import ExtractionStatistics, prepare_exclude_dirs

if TYPE_CHECKING:
    from collections.abc import Iterable, Sequence
    from pathlib import Path

    from ftl_extract.matcher import FluentKey


def extract(
    *,
    code_path: Path,
    output_path: Path,
    language: Sequence[str],
    i18n_keys: Iterable[str] = DEFAULT_I18N_KEYS,
    i18n_keys_append: Iterable[str] = (),
    i18n_keys_prefix: Iterable[str] = (),
    exclude_dirs: tuple[str, ...] = DEFAULT_EXCLUDE_DIRS,
    exclude_dirs_append: tuple[str, ...] = (),
    ignore_attributes: Iterable[str] = DEFAULT_IGNORE_ATTRIBUTES,
    append_ignore_attributes: Iterable[str] = (),
    ignore_kwargs: Iterable[str] = DEFAULT_IGNORE_KWARGS,
    comment_junks: bool = True,
    default_ftl_file: Path = DEFAULT_FTL_FILE,
    comment_keys_mode: str = COMMENT_KEYS_MODE[0],
    serializer: FluentSerializer | None = None,
    dry_run: bool = False,
) -> ExtractionStatistics:
    statistics = ExtractionStatistics()
    statistics.ftl_files_count = dict.fromkeys(language, 0)
    statistics.ftl_stored_keys_count = dict.fromkeys(language, 0)
    statistics.ftl_keys_updated = dict.fromkeys(language, 0)
    statistics.ftl_keys_added = dict.fromkeys(language, 0)
    statistics.ftl_keys_commented = dict.fromkeys(language, 0)

    exclude_dirs = prepare_exclude_dirs(
        exclude_dirs=exclude_dirs,
        exclude_dirs_append=exclude_dirs_append,
    )

    if i18n_keys_append:
        i18n_keys = (*i18n_keys, *i18n_keys_append)

    if append_ignore_attributes:
        ignore_attributes = (*ignore_attributes, *append_ignore_attributes)

    if serializer is None:
        serializer = FluentSerializer(with_junk=True)

    # Extract fluent keys from code
    in_code_fluent_keys = extract_fluent_keys(
        path=code_path,
        i18n_keys=i18n_keys,
        i18n_keys_prefix=i18n_keys_prefix,
        exclude_dirs=exclude_dirs,
        ignore_attributes=ignore_attributes,
        ignore_kwargs=ignore_kwargs,
        default_ftl_file=default_ftl_file,
        statistics=statistics,
    )
    statistics.ftl_in_code_keys_count = len(in_code_fluent_keys)

    for lang in language:
        # Import fluent keys and terms from existing FTL files
        stored_fluent_keys, stored_terms, leave_as_is = import_ftl_from_dir(
            path=output_path,
            locale=lang,
            statistics=statistics,
        )
        for fluent_key in stored_fluent_keys.values():
            fluent_key.path = fluent_key.path.relative_to(output_path / lang)

        for term in stored_terms.values():
            term.path = term.path.relative_to(output_path / lang)

        keys_to_comment: dict[str, FluentKey] = {}
        keys_to_add: dict[str, FluentKey] = {}

        # Find keys should be commented
        # Keys, that are not in code or not in their `path_` file
        # First step: find keys that have different paths
        for key, fluent_key in in_code_fluent_keys.items():
            if key in stored_fluent_keys and fluent_key.path != stored_fluent_keys[key].path:
                keys_to_comment[key] = stored_fluent_keys.pop(key)
                statistics.ftl_keys_commented[lang] += 1

                keys_to_add[key] = fluent_key
                statistics.ftl_keys_updated[lang] += 1

            elif key not in stored_fluent_keys:
                keys_to_add[key] = fluent_key
                statistics.ftl_keys_added[lang] += 1

            else:
                stored_fluent_keys[key].code_path = fluent_key.code_path

        # Second step: find keys that have different kwargs

        # Keys that are not in code but stored keys are depends on them
        depend_keys: set[str] = set()

        for key, fluent_key in in_code_fluent_keys.items():
            if key not in stored_fluent_keys:
                continue

            fluent_key_placeable_set = extract_kwargs(
                key=fluent_key,
                terms=stored_terms,
                all_fluent_keys=in_code_fluent_keys.copy(),
                depend_keys=depend_keys,
            )
            stored_fluent_key_placeable_set = extract_kwargs(
                key=stored_fluent_keys[key],
                terms=stored_terms,
                all_fluent_keys=stored_fluent_keys.copy(),
                depend_keys=depend_keys,
            )

            if fluent_key_placeable_set != stored_fluent_key_placeable_set:
                keys_to_comment[key] = stored_fluent_keys.pop(key)
                statistics.ftl_keys_commented[lang] += 1

                keys_to_add[key] = fluent_key
                statistics.ftl_keys_updated[lang] += 1

        # Third step: find keys that are not in code
        for key in stored_fluent_keys.keys() - in_code_fluent_keys.keys():
            if key in depend_keys:
                continue

            keys_to_comment[key] = stored_fluent_keys.pop(key)
            statistics.ftl_keys_commented[lang] += 1

        if comment_keys_mode == "comment":
            for fluent_key in keys_to_comment.values():
                comment_ftl_key(key=fluent_key, serializer=serializer)

        elif comment_keys_mode == "warn":
            for fluent_key in keys_to_comment.values():
                keys_to_add.pop(fluent_key.key, None)
                echo(
                    f"Key `{fluent_key.key}` with such kwargs in "
                    f"`{output_path / lang / fluent_key.path}` is not in code.",
                )

        # Comment Junk elements if needed
        if comment_junks is True:
            for fluent_key in leave_as_is:
                if isinstance(fluent_key.translation, fl_ast.Junk):
                    comment_ftl_key(key=fluent_key, serializer=serializer)
                    statistics.ftl_keys_commented[lang] += 1

        sorted_fluent_keys = sort_fluent_keys_by_path(fluent_keys=stored_fluent_keys)

        for path, keys in sort_fluent_keys_by_path(fluent_keys=keys_to_add).items():
            sorted_fluent_keys.setdefault(path, []).extend(keys)

        for path, keys in sort_fluent_keys_by_path(fluent_keys=keys_to_comment).items():
            sorted_fluent_keys.setdefault(path, []).extend(keys)

        for path, keys in sort_fluent_keys_by_path(fluent_keys=stored_terms).items():
            sorted_fluent_keys.setdefault(path, []).extend(keys)

        leave_as_is_with_path: dict[Path, list[FluentKey]] = {}

        for fluent_key in leave_as_is:
            leave_as_is_with_path.setdefault(
                fluent_key.path.relative_to(output_path / lang),
                [],
            ).append(fluent_key)

        for path, keys in sorted_fluent_keys.items():
            ftl, _ = generate_ftl(
                fluent_keys=keys,
                serializer=serializer,
                leave_as_is=leave_as_is_with_path.get(path, []),
            )
            if dry_run is True:
                echo(
                    f"[DRY-RUN] File {output_path / lang / path} has been saved. {len(keys)} "
                    f"keys found.",
                )
            else:
                _write(path=output_path / lang / path, ftl=ftl)
                echo(f"File {output_path / lang / path} has been saved. {len(keys)} keys found.")

            statistics.ftl_stored_keys_count[lang] += len(
                [key for key in keys if isinstance(key.translation, fl_ast.Message)],
            )

    return statistics


def _write(*, path: Path, ftl: str) -> None:
    """Write FTL to file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(ftl, encoding="utf-8")
