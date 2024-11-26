from pathlib import Path

import pytest
from fluent.syntax import FluentSerializer, ast

from ftl_extract.const import DEFAULT_FTL_FILE
from ftl_extract.matcher import FluentKey
from ftl_extract.process.serializer import generate_ftl


@pytest.fixture
def single_fluent_key() -> list[FluentKey]:
    return [
        FluentKey(
            code_path=Path("test.py"),
            key="greeting",
            translation=ast.Message(
                id=ast.Identifier("greeting"),
                value=ast.Pattern(elements=[ast.TextElement("Hello, world!")]),
            ),
            path=DEFAULT_FTL_FILE,
        ),
    ]


@pytest.fixture
def multiple_fluent_keys() -> list[FluentKey]:
    return [
        FluentKey(
            code_path=Path("test.py"),
            key="greeting",
            translation=ast.Message(
                id=ast.Identifier("greeting"),
                value=ast.Pattern(elements=[ast.TextElement("Hello, world!")]),
            ),
            path=DEFAULT_FTL_FILE,
        ),
        FluentKey(
            code_path=Path("test.py"),
            key="farewell",
            translation=ast.Message(
                id=ast.Identifier("farewell"),
                value=ast.Pattern(elements=[ast.TextElement("Goodbye, world!")]),
            ),
            path=DEFAULT_FTL_FILE,
        ),
    ]


@pytest.fixture
def empty_fluent_keys() -> list[FluentKey]:
    return []


def test_custom_serializer_produces_correct_ftl_for_single_key(
    single_fluent_key: list[FluentKey],
) -> None:
    ftl_string, resource = generate_ftl(
        fluent_keys=single_fluent_key,
        serializer=FluentSerializer(),
        leave_as_is=[],
    )
    assert "greeting = Hello, world!" in ftl_string
    assert len(resource.body) == 1


def test_custom_serializer_produces_correct_ftl_for_multiple_keys(
    multiple_fluent_keys: list[FluentKey],
) -> None:
    ftl_string, resource = generate_ftl(
        fluent_keys=multiple_fluent_keys,
        serializer=FluentSerializer(),
        leave_as_is=[],
    )
    assert "greeting = Hello, world!" in ftl_string
    assert "farewell = Goodbye, world!" in ftl_string
    assert len(resource.body) == 2  # noqa: PLR2004


def test_custom_serializer_handles_empty_fluent_keys_list_properly(
    empty_fluent_keys: list[FluentKey],
) -> None:
    ftl_string, resource = generate_ftl(
        fluent_keys=empty_fluent_keys,
        serializer=FluentSerializer(),
        leave_as_is=[],
    )
    assert ftl_string == ""
    assert resource.body is None or len(resource.body) == 0


def test_generate_ftl_includes_leave_as_is_elements() -> None:
    ftl_string, resource = generate_ftl(
        fluent_keys=[
            FluentKey(
                code_path=Path("test.py"),
                key="test_key",
                translation=ast.Message(
                    id=ast.Identifier("test_message"),
                    value=ast.Pattern(elements=[ast.TextElement("Test message content")]),
                ),
                path=DEFAULT_FTL_FILE,
            ),
        ],
        serializer=FluentSerializer(with_junk=True),
        leave_as_is=[
            FluentKey(
                code_path=Path(),
                key="",
                translation=ast.Comment(content="This is a comment"),
                path=DEFAULT_FTL_FILE,
            ),
            FluentKey(
                code_path=Path(),
                key="",
                translation=ast.GroupComment(content="This is a group comment"),
                path=DEFAULT_FTL_FILE,
            ),
            FluentKey(
                code_path=Path(),
                key="",
                translation=ast.ResourceComment(content="This is a resource comment"),
                path=DEFAULT_FTL_FILE,
            ),
            FluentKey(
                code_path=Path(),
                key="",
                translation=ast.Term(
                    id=ast.Identifier("test_term"),
                    value=ast.Pattern(elements=[ast.TextElement("Test term content")]),
                ),
                path=DEFAULT_FTL_FILE,
            ),
            FluentKey(
                code_path=Path(),
                key="",
                translation=ast.Junk(
                    content="This is junk content",
                ),
                path=DEFAULT_FTL_FILE,
            ),
        ],
    )
    assert "This is a comment" in ftl_string
    assert "This is a group comment" in ftl_string
    assert "This is a resource comment" in ftl_string
    assert "test_term = Test term content" in ftl_string
    assert "This is junk content" in ftl_string
    assert "Test message content" in ftl_string
