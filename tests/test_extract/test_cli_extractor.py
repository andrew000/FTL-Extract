from pathlib import Path
from typing import cast
from unittest.mock import MagicMock, patch

import click.testing
import pytest
from click import BaseCommand
from fluent.syntax import ast

from ftl_extract.cli import cli_extract
from ftl_extract.ftl_extractor import extract
from ftl_extract.matcher import FluentKey


@pytest.fixture()
def mock_fluent_key(tmp_path: Path) -> FluentKey:
    mock = MagicMock(spec=FluentKey)
    mock.code_path = tmp_path / "code"
    mock.translation = MagicMock(spec=ast.Message)

    mock.translation.id = MagicMock(spec=ast.Identifier)
    mock.translation.id.name = "key-1"

    text_element = MagicMock(spec=ast.TextElement)
    text_element.value = "key-1"
    mock.translation.value = MagicMock(spec=ast.Pattern)
    mock.translation.value.elements = [text_element]

    mock.translation.attributes = []

    mock.translation.comment = MagicMock(spec=ast.Comment)
    mock.translation.comment.content = "Comment"

    return mock


@pytest.fixture()
def setup_environment(tmp_path: Path) -> tuple[Path, Path]:
    code_path = tmp_path / "code"
    output_path = tmp_path / "output"
    code_path.mkdir()
    output_path.mkdir()
    return code_path, output_path


@pytest.fixture()
def runner() -> click.testing.CliRunner:
    return click.testing.CliRunner()


@pytest.fixture()
def mock_extract_function() -> patch:
    with patch("ftl_extract.cli.extract") as mock:
        yield mock


def test_extract_with_beauty_enabled(
    setup_environment: tuple[Path, Path],
    mock_fluent_key: FluentKey,
) -> None:
    code_path, output_path = setup_environment

    with (
        patch(
            "ftl_extract.ftl_extractor.extract_fluent_keys", return_value={"key-1": mock_fluent_key}
        ),
        patch(
            "ftl_extract.ftl_extractor.import_ftl_from_dir",
            return_value=({"key-1": mock_fluent_key}, []),
        ),
        patch(
            "ftl_extract.ftl_extractor.generate_ftl", return_value=("key-1 = key-1", None)
        ) as mock_generate_ftl,
    ):
        extract(code_path, output_path, ("en",), ("i18n",), beauty=True)
        mock_generate_ftl.assert_called()


def test_extract_with_keys_to_comment_and_add(
    setup_environment: tuple[Path, Path],
    mock_fluent_key: FluentKey,
) -> None:
    code_path, output_path = setup_environment

    # Adjust the path to be relative to `output_path / "en"`, ensuring it's a valid subpath
    stored_fluent_key_path = (output_path / "en").resolve().joinpath("different/path.ftl")
    mock_fluent_key.path = Path("some/path.ftl")  # Path in code

    with (
        patch(
            "ftl_extract.ftl_extractor.extract_fluent_keys", return_value={"key-1": mock_fluent_key}
        ),
        patch(
            "ftl_extract.ftl_extractor.import_ftl_from_dir",
            return_value=({"key-1": MagicMock(spec=FluentKey, path=stored_fluent_key_path)}, []),
        ),
        patch("ftl_extract.ftl_extractor.comment_ftl_key") as mock_comment_ftl_key,
        patch(
            "ftl_extract.ftl_extractor.generate_ftl", return_value=("generated ftl", None)
        ) as mock_generate_ftl,
    ):
        extract(code_path, output_path, ("en",), ("i18n",), beauty=False)
        mock_comment_ftl_key.assert_called()
        mock_generate_ftl.assert_called()


def test_extract_with_keys_only_to_add(
    setup_environment: tuple[Path, Path],
    mock_fluent_key: FluentKey,
) -> None:
    code_path, output_path = setup_environment

    # Correctly set the path to be recognized as a subpath of `output_path / "en"`
    mock_fluent_key.path = output_path / "en" / "new" / "path.ftl"

    with (
        patch(
            "ftl_extract.ftl_extractor.extract_fluent_keys", return_value={"key-2": mock_fluent_key}
        ),
        patch(
            "ftl_extract.ftl_extractor.import_ftl_from_dir",
            return_value=({"key-1": mock_fluent_key}, []),
        ),
        patch(
            "ftl_extract.ftl_extractor.generate_ftl", return_value=("generated ftl", None)
        ) as mock_generate_ftl,
    ):
        extract(code_path, output_path, ("en",), ("i18n",), beauty=False)
        mock_generate_ftl.assert_called()


def test_extraction_with_valid_paths_succeeds(
    runner: click.testing.CliRunner,
    mock_extract_function: patch,
    tmp_path: Path,
) -> None:
    tmp_path.joinpath("path/to/code").mkdir(parents=True)
    code_path = tmp_path.joinpath("path/to/code")
    output_path = tmp_path.joinpath("path/to/output")

    result = runner.invoke(
        cast(BaseCommand, cli_extract), [code_path.as_posix(), output_path.as_posix()]
    )
    assert result.exit_code == 0
    assert f"Extracting from {code_path}..." in result.output
    mock_extract_function.assert_called_once()


def test_extraction_with_multiple_languages_handles_all(
    runner: click.testing.CliRunner,
    mock_extract_function: patch,
    tmp_path: Path,
) -> None:
    tmp_path.joinpath("path/to/code").mkdir(parents=True)
    code_path = tmp_path.joinpath("path/to/code")
    output_path = tmp_path.joinpath("path/to/output")

    result = runner.invoke(
        cast(BaseCommand, cli_extract),
        [code_path.as_posix(), output_path.as_posix(), "-l", "en", "-l", "fr"],
    )
    assert result.exit_code == 0
    assert mock_extract_function.call_args[1]["language"] == ("en", "fr")


def test_extraction_with_beautify_option_enables_beautification(
    runner: click.testing.CliRunner,
    mock_extract_function: patch,
    tmp_path: Path,
) -> None:
    tmp_path.joinpath("path/to/code").mkdir(parents=True)
    code_path = tmp_path.joinpath("path/to/code")
    output_path = tmp_path.joinpath("path/to/output")

    result = runner.invoke(
        cast(BaseCommand, cli_extract), [code_path.as_posix(), output_path.as_posix(), "--beauty"]
    )
    assert result.exit_code == 0
    assert mock_extract_function.call_args[1]["beauty"] is True


def test_extraction_with_nonexistent_code_path_fails(runner: click.testing.CliRunner) -> None:
    result = runner.invoke(cast(BaseCommand, cli_extract), ["nonexistent/path", "path/to/output"])
    assert result.exit_code != 0
    assert "Invalid value for 'CODE_PATH'" in result.output


def test_extraction_with_invalid_i18n_keys_ignores_them(
    runner: click.testing.CliRunner,
    mock_extract_function: patch,
    tmp_path: Path,
) -> None:
    tmp_path.joinpath("path/to/code").mkdir(parents=True)
    code_path = tmp_path.joinpath("path/to/code")
    output_path = tmp_path.joinpath("path/to/output")

    result = runner.invoke(
        cast(BaseCommand, cli_extract),
        [code_path.as_posix(), output_path.as_posix(), "-k", "nonexistent_key"],
    )
    assert result.exit_code == 0
    assert mock_extract_function.call_args[1]["i18n_keys"] == ("nonexistent_key",)
