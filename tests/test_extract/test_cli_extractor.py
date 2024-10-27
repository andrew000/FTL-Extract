import ast
from pathlib import Path
from typing import cast
from unittest.mock import MagicMock, patch

import click.testing
import pytest
from click import BaseCommand
from fluent.syntax import FluentSerializer
from fluent.syntax import ast as fl_ast

from ftl_extract.cli import cli_extract
from ftl_extract.const import DEFAULT_FTL_FILE
from ftl_extract.ftl_extractor import extract
from ftl_extract.matcher import FluentKey, I18nMatcher


@pytest.fixture
def mock_fluent_key(tmp_path: Path) -> FluentKey:
    mock = MagicMock(spec=FluentKey)
    mock.code_path = tmp_path / "code"
    mock.translation = MagicMock(spec=fl_ast.Message)

    mock.translation.id = MagicMock(spec=fl_ast.Identifier)
    mock.translation.id.name = "key-1"

    text_element = MagicMock(spec=fl_ast.TextElement)
    text_element.value = "key-1"
    mock.translation.value = MagicMock(spec=fl_ast.Pattern)
    mock.translation.value.elements = [text_element]

    mock.translation.attributes = []

    mock.translation.comment = MagicMock(spec=fl_ast.Comment)
    mock.translation.comment.content = "Comment"

    return mock


@pytest.fixture
def setup_environment(tmp_path: Path) -> tuple[Path, Path]:
    code_path = tmp_path / "code"
    output_path = tmp_path / "output"
    code_path.mkdir()
    output_path.mkdir()
    return code_path, output_path


@pytest.fixture
def runner() -> click.testing.CliRunner:
    return click.testing.CliRunner()


@pytest.fixture
def mock_extract_function() -> patch:
    with patch("ftl_extract.cli.extract") as mock:
        yield mock


@pytest.fixture
def mock_leave_as_is() -> list:
    return [
        MagicMock(spec=fl_ast.Comment),
        MagicMock(spec=fl_ast.GroupComment),
        MagicMock(spec=fl_ast.ResourceComment),
        MagicMock(spec=fl_ast.Term),
        MagicMock(spec=fl_ast.Junk),
    ]


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
            "ftl_extract.ftl_extractor.extract_fluent_keys",
            return_value={"key-1": mock_fluent_key},
        ),
        patch(
            "ftl_extract.ftl_extractor.import_ftl_from_dir",
            return_value=({"key-1": MagicMock(spec=FluentKey, path=stored_fluent_key_path)}, []),
        ),
        patch("ftl_extract.ftl_extractor.comment_ftl_key") as mock_comment_ftl_key,
        patch(
            "ftl_extract.ftl_extractor.generate_ftl",
            return_value=("generated ftl", None),
        ) as mock_generate_ftl,
    ):
        extract(code_path, output_path, ("en",), ("i18n",))
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
            "ftl_extract.ftl_extractor.extract_fluent_keys",
            return_value={"key-2": mock_fluent_key},
        ),
        patch(
            "ftl_extract.ftl_extractor.import_ftl_from_dir",
            return_value=({"key-1": mock_fluent_key}, []),
        ),
        patch(
            "ftl_extract.ftl_extractor.generate_ftl",
            return_value=("generated ftl", None),
        ) as mock_generate_ftl,
    ):
        extract(code_path, output_path, ("en",), ("i18n",))
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
        cast(BaseCommand, cli_extract),
        [code_path.as_posix(), output_path.as_posix()],
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


def test_comment_junk_elements_if_needed(setup_environment: tuple[Path, Path]) -> None:
    code_path, output_path = setup_environment

    mock_junk_key = MagicMock(spec=FluentKey)
    mock_junk_key.translation = MagicMock(spec=fl_ast.Junk)
    mock_junk_key.path = MagicMock(spec=Path)
    mock_serializer = MagicMock(spec=FluentSerializer)

    with (
        patch("ftl_extract.ftl_extractor.extract_fluent_keys", return_value={}),
        patch("ftl_extract.ftl_extractor.import_ftl_from_dir", return_value=({}, [mock_junk_key])),
        patch("ftl_extract.ftl_extractor.comment_ftl_key") as mock_comment_ftl_key,
        patch("fluent.syntax.serializer.FluentSerializer", return_value=mock_serializer),
    ):
        extract(
            code_path,
            output_path,
            ("en",),
            ("i18n",),
            comment_junks=True,
            serializer=mock_serializer,
        )
        mock_comment_ftl_key.assert_called_once_with(mock_junk_key, mock_serializer)


def test_expand_ignore_attributes_updates_ignore_attributes(
    setup_environment: tuple[Path, Path],
) -> None:
    code_path, output_path = setup_environment
    initial_ignore_attributes = ["attr1", "attr2"]
    expand_ignore_attributes = ["attr3", "attr4"]
    expected_ignore_attributes = frozenset({"attr1", "attr2", "attr3", "attr4"})

    with (
        patch("ftl_extract.ftl_extractor.extract_fluent_keys", return_value={}),
        patch("ftl_extract.ftl_extractor.import_ftl_from_dir", return_value=({}, [])),
        patch("ftl_extract.ftl_extractor.comment_ftl_key"),
        patch("ftl_extract.ftl_extractor.generate_ftl", return_value=("generated ftl", None)),
    ):
        extract(
            code_path,
            output_path,
            ("en",),
            ("i18n",),
            ignore_attributes=initial_ignore_attributes,
            expand_ignore_attributes=expand_ignore_attributes,
        )

        assert (
            frozenset(initial_ignore_attributes) | frozenset(expand_ignore_attributes)
            == expected_ignore_attributes
        )


def test_stored_fluent_keys_code_path_update(setup_environment: tuple[Path, Path]) -> None:
    code_path, output_path = setup_environment
    mock_fluent_key = MagicMock(spec=FluentKey)
    mock_fluent_key.path = Path("_default.ftl")
    mock_fluent_key.code_path = code_path / "some_code_path.py"

    stored_fluent_key = MagicMock(spec=FluentKey)
    stored_fluent_key.path = Path(output_path / "en" / "_default.ftl")
    stored_fluent_key.code_path = None

    in_code_fluent_keys = {"key-1": mock_fluent_key}
    stored_fluent_keys = {"key-1": stored_fluent_key}

    with (
        patch("ftl_extract.ftl_extractor.extract_fluent_keys", return_value=in_code_fluent_keys),
        patch(
            "ftl_extract.ftl_extractor.import_ftl_from_dir",
            return_value=(stored_fluent_keys, []),
        ),
        patch("ftl_extract.ftl_extractor.extract_kwargs", return_value=set()),
        patch("ftl_extract.ftl_extractor.comment_ftl_key"),
        patch("ftl_extract.ftl_extractor.generate_ftl", return_value=("generated ftl", None)),
    ):
        extract(
            code_path,
            output_path,
            ("en",),
            ("i18n",),
        )

        assert stored_fluent_keys["key-1"].code_path == mock_fluent_key.code_path


def test_keys_to_comment_and_add_on_different_kwargs(setup_environment: tuple[Path, Path]) -> None:
    code_path, output_path = setup_environment
    mock_fluent_key = MagicMock(spec=FluentKey)
    mock_fluent_key.path = Path("_default.ftl")
    mock_fluent_key.code_path = code_path / "some_code_path.py"

    stored_fluent_key = MagicMock(spec=FluentKey)
    stored_fluent_key.path = Path(output_path / "en" / "_default.ftl")
    stored_fluent_key.code_path = None

    in_code_fluent_keys = {"key-1": mock_fluent_key}
    stored_fluent_keys = {"key-1": stored_fluent_key}

    with (
        patch("ftl_extract.ftl_extractor.extract_fluent_keys", return_value=in_code_fluent_keys),
        patch(
            "ftl_extract.ftl_extractor.import_ftl_from_dir",
            return_value=(stored_fluent_keys, []),
        ),
        patch("ftl_extract.ftl_extractor.extract_kwargs", side_effect=[{"arg1"}, {"arg2"}]),
        patch("ftl_extract.ftl_extractor.comment_ftl_key"),
        patch("ftl_extract.ftl_extractor.generate_ftl", return_value=("generated ftl", None)),
    ):
        extract(
            code_path,
            output_path,
            ("en",),
            ("i18n",),
        )

        assert "key-1" not in stored_fluent_keys
        assert "key-1" in in_code_fluent_keys
        assert in_code_fluent_keys["key-1"] == mock_fluent_key


def test_i18n_matcher_skips_call_with_no_args(setup_environment: tuple[Path, Path]) -> None:
    code_path, output_path = setup_environment
    matcher = I18nMatcher(code_path, default_ftl_file=DEFAULT_FTL_FILE)

    node = ast.Call(func=ast.Attribute(value=ast.Name(id="i18n"), attr="get"), args=[], keywords=[])
    matcher.visit_Call(node)

    assert len(matcher.fluent_keys) == 0


def test_generic_visit_called_on_else_block(setup_environment: tuple[Path, Path]) -> None:
    code_path, output_path = setup_environment
    matcher = I18nMatcher(code_path, default_ftl_file=DEFAULT_FTL_FILE)

    node = ast.Call(
        func=ast.Attribute(value=ast.Name(id="i18n"), attr="get"),
        args=[ast.Name(id="i18n")],
        keywords=[],
    )

    with patch.object(matcher, "generic_visit", wraps=matcher.generic_visit) as mock_generic_visit:
        matcher.visit_Call(node)
        mock_generic_visit.assert_called()


def test_generic_visit_called_when_attr_in_ignore_attributes(
    setup_environment: tuple[Path, Path],
) -> None:
    code_path, output_path = setup_environment
    matcher = I18nMatcher(
        code_path,
        default_ftl_file=DEFAULT_FTL_FILE,
        ignore_attributes={"ignore_this"},
    )

    # Create a mock AST node for a function call with an attribute in ignore_attributes
    node = ast.Call(
        func=ast.Attribute(
            value=ast.Name(id="i18n", ctx=ast.Load()),
            attr="ignore_this",
            ctx=ast.Load(),
        ),
        args=[ast.Constant(value="key")],
        keywords=[],
    )

    with patch.object(matcher, "generic_visit", wraps=matcher.generic_visit) as mock_generic_visit:
        matcher.visit_Call(node)
        mock_generic_visit.assert_called_with(node.args[0])

        assert len(matcher.fluent_keys) == 0


def test_i18n_matcher_skips_call_with_no_args_in_elif(setup_environment: tuple[Path, Path]) -> None:
    code_path, output_path = setup_environment
    matcher = I18nMatcher(code_path, default_ftl_file=DEFAULT_FTL_FILE)

    node = ast.Call(func=ast.Name(id="i18n", ctx=ast.Load()), args=[], keywords=[])
    matcher.visit_Call(node)

    assert len(matcher.fluent_keys) == 0
