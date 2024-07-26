from pathlib import Path
from typing import Final

import pytest

from ftl_extract.code_extractor import extract_fluent_keys
from ftl_extract.const import IGNORE_ATTRIBUTES
from ftl_extract.exceptions import (
    FTLExtractorDifferentPathsError,
    FTLExtractorDifferentTranslationError,
)

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
        extract_fluent_keys(tmp_path, "i18n", IGNORE_ATTRIBUTES)


def test_extract_similar_fluent_keys_in_different_paths_from_different_py_files(
    tmp_path: Path,
) -> None:
    (tmp_path / "test.py").write_text(CONTENT_2_1)

    (tmp_path / "test2.py").write_text(CONTENT_2_2)

    with pytest.raises(FTLExtractorDifferentPathsError):
        extract_fluent_keys(tmp_path, "i18n", IGNORE_ATTRIBUTES)


def test_extract_similar_fluent_keys_with_different_translation_one_py_file(tmp_path: Path) -> None:
    (tmp_path / "test.py").write_text(CONTENT_3)

    with pytest.raises(FTLExtractorDifferentTranslationError):
        extract_fluent_keys(tmp_path, "i18n", IGNORE_ATTRIBUTES)


def test_extract_similar_fluent_keys_with_different_translation_different_py_files(
    tmp_path: Path,
) -> None:
    (tmp_path / "test.py").write_text(CONTENT_4_1)

    (tmp_path / "test2.py").write_text(CONTENT_4_2)

    with pytest.raises(FTLExtractorDifferentTranslationError):
        extract_fluent_keys(tmp_path, "i18n", IGNORE_ATTRIBUTES)
