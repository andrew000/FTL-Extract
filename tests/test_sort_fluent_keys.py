from pathlib import Path
from unittest.mock import Mock

from ftl_extract.code_extractor import sort_fluent_keys_by_path
from ftl_extract.matcher import FluentKey


def fluent_key_mock(path: str) -> FluentKey:
    mock = Mock(spec=FluentKey)
    mock.path = Path(path)
    return mock


def test_sort_fluent_keys_by_single_path() -> None:
    fluent_keys = {
        "key-1": fluent_key_mock("path/to/file1.ftl"),
    }
    expected = {
        Path("path/to/file1.ftl"): [fluent_keys["key-1"]],
    }
    assert sort_fluent_keys_by_path(fluent_keys=fluent_keys) == expected


def test_sort_fluent_keys_by_multiple_paths() -> None:
    fluent_keys = {
        "key-1": fluent_key_mock("path/to/file1.ftl"),
        "key-2": fluent_key_mock("path/to/file2.ftl"),
    }
    expected = {
        Path("path/to/file1.ftl"): [fluent_keys["key-1"]],
        Path("path/to/file2.ftl"): [fluent_keys["key-2"]],
    }
    assert sort_fluent_keys_by_path(fluent_keys=fluent_keys) == expected


def test_sort_fluent_keys_by_same_path() -> None:
    fluent_keys = {
        "key-1": fluent_key_mock("path/to/file.ftl"),
        "key-2": fluent_key_mock("path/to/file.ftl"),
    }
    expected = {
        Path("path/to/file.ftl"): [fluent_keys["key-1"], fluent_keys["key-2"]],
    }
    assert sort_fluent_keys_by_path(fluent_keys=fluent_keys) == expected


def test_sort_fluent_keys_empty_dict() -> None:
    fluent_keys = {}
    expected = {}
    assert sort_fluent_keys_by_path(fluent_keys=fluent_keys) == expected


def test_sort_fluent_keys_with_nonexistent_path() -> None:
    fluent_keys = {
        "key-1": fluent_key_mock("nonexistent/path.ftl"),
    }
    expected = {
        Path("nonexistent/path.ftl"): [fluent_keys["key-1"]],
    }
    assert sort_fluent_keys_by_path(fluent_keys=fluent_keys) == expected
