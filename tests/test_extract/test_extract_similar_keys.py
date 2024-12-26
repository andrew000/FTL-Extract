from pathlib import Path
from pprint import pformat
from typing import Final
from unittest.mock import Mock, patch

import pytest
from fluent.syntax import ast

from ftl_extract.code_extractor import extract_fluent_keys
from ftl_extract.const import DEFAULT_FTL_FILE, IGNORE_ATTRIBUTES, IGNORE_KWARGS
from ftl_extract.exceptions import (
    FTLExtractorCantFindReferenceError,
    FTLExtractorCantFindTermError,
    FTLExtractorDifferentPathsError,
    FTLExtractorDifferentTranslationError,
)
from ftl_extract.utils import to_json_no_span

CONTENT_1: Final[str] = """
# For `test_extract_similar_keys_in_different_paths_from_one_py_file` test.
def test(i18n):
    i18n.get("key-1", _path="content_2/file_1.ftl")
    i18n.get("key-1", _path="content_2/file_2.ftl")
"""

CONTENT_2_1: Final[str] = """
# For `test_extract_similar_fluent_keys_in_different_paths_from_different_py_files` test.
def test(i18n):
    i18n.get("key-1", arg_1="arg-1", _path="content_3/file_1.ftl")
"""

CONTENT_2_2: Final[str] = """
# For `test_extract_similar_fluent_keys_in_different_paths_from_different_py_files` test.
def test(i18n):
    i18n.get("key-1", arg_1="arg-1", _path="content_3/file_2.ftl")
"""
CONTENT_3: Final[str] = """
# For `test_extract_similar_fluent_keys_with_different_kwargs_one_py_file` test.
def test(i18n):
    i18n.get("key-1", arg_1="arg-1")
    i18n.get("key-1", arg_2="arg-2")
"""

CONTENT_4_1: Final[str] = """
# For `test_extract_similar_fluent_keys_with_different_kwargs_different_py_files` test.
def test(i18n):
    i18n.get("key-1", arg_1="arg-1")
"""

CONTENT_4_2: Final[str] = """
# For `test_extract_similar_fluent_keys_with_different_kwargs_different_py_files` test.
def test(i18n):
    i18n.get("key-1", arg_2="arg-2")
"""


def test_extract_similar_keys_in_different_paths_from_one_py_file(tmp_path: Path) -> None:
    (tmp_path / "test.py").write_text(CONTENT_1)

    with pytest.raises(FTLExtractorDifferentPathsError):
        extract_fluent_keys(
            path=tmp_path,
            i18n_keys="i18n",
            ignore_attributes=IGNORE_ATTRIBUTES,
            ignore_kwargs=IGNORE_KWARGS,
            default_ftl_file=DEFAULT_FTL_FILE,
        )


def test_extract_similar_fluent_keys_in_different_paths_from_different_py_files(
    tmp_path: Path,
) -> None:
    (tmp_path / "test.py").write_text(CONTENT_2_1)

    (tmp_path / "test2.py").write_text(CONTENT_2_2)

    with pytest.raises(FTLExtractorDifferentPathsError):
        extract_fluent_keys(
            path=tmp_path,
            i18n_keys="i18n",
            ignore_attributes=IGNORE_ATTRIBUTES,
            ignore_kwargs=IGNORE_KWARGS,
            default_ftl_file=DEFAULT_FTL_FILE,
        )


def test_different_translation_error_with_debug_var() -> None:
    with patch("ftl_extract.exceptions.environ") as environ:
        environ.get = Mock(return_value=True)
        identifier = "hello"
        current_translation = ast.Message(
            id=ast.Identifier(identifier),
            value=ast.Pattern([ast.TextElement("Hello, world!")]),
        )
        new_translation = ast.Message(
            id=ast.Identifier(identifier),
            value=ast.Pattern([ast.TextElement("Hello, universe!")]),
        )
        with pytest.raises(FTLExtractorDifferentTranslationError) as exc_info:
            raise FTLExtractorDifferentTranslationError(
                identifier,
                current_translation,
                new_translation,
            )
        assert (
            exc_info.value.args[0]
            == f"Translation {identifier!r} already exists with different elements:\n"
            f"current_translation: {pformat(current_translation.to_json(fn=to_json_no_span))}\n!= "
            f"new_translation: {pformat(new_translation.to_json(fn=to_json_no_span))}"
        )


def test_extract_similar_fluent_keys_with_different_translation_one_py_file(tmp_path: Path) -> None:
    (tmp_path / "test.py").write_text(CONTENT_3)

    with pytest.raises(FTLExtractorDifferentTranslationError):
        extract_fluent_keys(
            path=tmp_path,
            i18n_keys="i18n",
            ignore_attributes=IGNORE_ATTRIBUTES,
            ignore_kwargs=IGNORE_KWARGS,
            default_ftl_file=DEFAULT_FTL_FILE,
        )


def test_extract_similar_fluent_keys_with_different_translation_different_py_files(
    tmp_path: Path,
) -> None:
    (tmp_path / "test.py").write_text(CONTENT_4_1)

    (tmp_path / "test2.py").write_text(CONTENT_4_2)

    with pytest.raises(FTLExtractorDifferentTranslationError):
        extract_fluent_keys(
            path=tmp_path,
            i18n_keys="i18n",
            ignore_attributes=IGNORE_ATTRIBUTES,
            ignore_kwargs=IGNORE_KWARGS,
            default_ftl_file=DEFAULT_FTL_FILE,
        )


def test_ftl_extractor_cant_find_reference_error() -> None:
    key = "example_key"
    key_path = Path("/path/to/locale/en/example.ftl")
    reference_key = "missing_reference"
    error = FTLExtractorCantFindReferenceError(key, key_path, reference_key)
    assert str(error) == f"Can't find reference {reference_key!r} for key {key!r} at {key_path}"
    assert error.key == key
    assert error.key_path == key_path
    assert error.reference_key == reference_key


def test_ftl_extractor_cant_find_term_error() -> None:
    key = "example_key"
    key_path = Path("/path/to/locale/en/example.ftl")
    term_key = "example_term"

    error = FTLExtractorCantFindTermError(key, key_path, term_key)

    assert error.key == key
    assert error.key_path == key_path
    assert error.term_key == term_key
    assert str(error) == f"Can't find term {term_key!r} for key {key!r} at {key_path}"
