from pathlib import Path
from unittest.mock import patch

import pytest
from fluent.syntax import ast

from ftl_extract.ftl_importer import import_from_ftl, import_ftl_from_dir


@pytest.fixture
def mock_ftl_content() -> str:
    return """
# Simple FTL file
## Group Comment
### Resource Comment
-term = Term value
hello = Hello, world!
welcome = Welcome, { $name }!
Junk
"""


def test_import_from_ftl_with_valid_ftl_file(mock_ftl_content: str) -> None:
    with patch("pathlib.Path.read_text", return_value=mock_ftl_content):
        keys, _, resource, _ = import_from_ftl(
            path=Path("/path/to/locale/en/example.ftl"),
            locale="en",
        )
        assert "hello" in keys
        assert "welcome" in keys
        assert len(resource.body) == 7  # noqa: PLR2004


def test_import_from_ftl_with_empty_ftl_file() -> None:
    with patch("pathlib.Path.read_text", return_value=""):
        keys, _, resource, _ = import_from_ftl(
            path=Path("/path/to/locale/en/empty.ftl"),
            locale="en",
        )
        assert len(keys) == 0
        assert len(resource.body) == 0


def test_import_ftl_from_dir_with_multiple_files(tmp_path: Path, mock_ftl_content: str) -> None:
    (tmp_path / "en").mkdir(parents=True)
    file1 = tmp_path / "en" / "file1.ftl"
    file2 = tmp_path / "en" / "file2.ftl"
    file1.write_text(mock_ftl_content)
    file2.write_text(mock_ftl_content)

    keys, _, _ = import_ftl_from_dir(path=tmp_path, locale="en")
    assert len(keys) == 2  # noqa: PLR2004


def test_import_ftl_from_dir_with_no_ftl_files(tmp_path: Path) -> None:
    (tmp_path / "en").mkdir(parents=True)
    keys, _, _ = import_ftl_from_dir(path=tmp_path, locale="en")
    assert len(keys) == 0


def test_import_ftl_from_dir_with_nonexistent_directory() -> None:
    with pytest.raises(FileNotFoundError):
        import_ftl_from_dir(
            path=Path("/path/to/nonexistent/dir"),
            locale="en",
        )


def test_import_from_ftl_appends_non_message_entries_correctly(mock_ftl_content: str) -> None:
    with patch("pathlib.Path.read_text", return_value=mock_ftl_content):
        _, _, _, leave_as_is = import_from_ftl(
            path=Path("/path/to/locale/en/various_entries.ftl"),
            locale="en",
        )
        assert len(leave_as_is) == 4  # noqa: PLR2004
        assert isinstance(leave_as_is[0].translation, ast.Comment)
        assert isinstance(leave_as_is[1].translation, ast.GroupComment)
        assert isinstance(leave_as_is[2].translation, ast.ResourceComment)
        assert isinstance(leave_as_is[3].translation, ast.Junk)
