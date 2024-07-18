from __future__ import annotations

from typing import TYPE_CHECKING

from click import echo
from fluent.syntax import FluentSerializer

from ftl_extract import extract_fluent_keys
from ftl_extract.code_extractor import sort_fluent_keys_by_path
from ftl_extract.ftl_importer import import_ftl_from_dir
from ftl_extract.process.commentator import comment_ftl_key
from ftl_extract.process.kwargs_extractor import extract_kwargs
from ftl_extract.process.serializer import BeautyFluentSerializer, generate_ftl

if TYPE_CHECKING:
    from pathlib import Path

    from ftl_extract.matcher import FluentKey


def extract(
    code_path: Path,
    output_path: Path,
    language: tuple[str, ...],
    i18n_keys: tuple[str, ...],
    beauty: bool = False,
) -> None:
    serializer: FluentSerializer | BeautyFluentSerializer

    if beauty is True:
        serializer = BeautyFluentSerializer(with_junk=True)
    else:
        serializer = FluentSerializer(with_junk=True)

    # Extract fluent keys from code
    in_code_fluent_keys = extract_fluent_keys(code_path, i18n_keys)

    for lang in language:
        # Import fluent keys from existing FTL files
        stored_fluent_keys, leave_as_is = import_ftl_from_dir(output_path, lang)
        for fluent_key in stored_fluent_keys.values():
            fluent_key.path = fluent_key.path.relative_to(output_path / lang)

        keys_to_comment: dict[str, FluentKey] = {}
        keys_to_add: dict[str, FluentKey] = {}

        # Find keys should be commented
        # Keys, that are not in code or not in their `path_` file
        # First step: find keys that have different paths
        for key, fluent_key in in_code_fluent_keys.items():
            if key in stored_fluent_keys and fluent_key.path != stored_fluent_keys[key].path:
                keys_to_comment[key] = stored_fluent_keys.pop(key)
                keys_to_add[key] = fluent_key

            elif key not in stored_fluent_keys:
                keys_to_add[key] = fluent_key

            else:
                stored_fluent_keys[key].code_path = fluent_key.code_path

        # Second step: find keys that have different kwargs
        for key, fluent_key in in_code_fluent_keys.items():
            if key not in stored_fluent_keys:
                continue

            fluent_key_placeable_set = extract_kwargs(fluent_key)
            stored_fluent_key_placeable_set = extract_kwargs(stored_fluent_keys[key])

            if fluent_key_placeable_set != stored_fluent_key_placeable_set:
                keys_to_comment[key] = stored_fluent_keys.pop(key)
                keys_to_add[key] = fluent_key

        # Third step: find keys that are not in code
        for key in stored_fluent_keys.keys() - in_code_fluent_keys.keys():
            keys_to_comment[key] = stored_fluent_keys.pop(key)

        for fluent_key in keys_to_comment.values():
            comment_ftl_key(fluent_key, serializer)

        sorted_fluent_keys = sort_fluent_keys_by_path(stored_fluent_keys)

        for path, keys in sort_fluent_keys_by_path(keys_to_add).items():
            sorted_fluent_keys.setdefault(path, []).extend(keys)

        for path, keys in sort_fluent_keys_by_path(keys_to_comment).items():
            sorted_fluent_keys.setdefault(path, []).extend(keys)

        for path, keys in sorted_fluent_keys.items():
            ftl, _ = generate_ftl(keys, serializer=serializer, leave_as_is=leave_as_is)
            (output_path / lang / path).parent.mkdir(parents=True, exist_ok=True)
            (output_path / lang / path).write_text(ftl, encoding="utf-8")
            echo(f"File {output_path / lang / path} has been saved. {len(keys)} keys updated.")
