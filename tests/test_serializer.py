from pathlib import Path

import pytest
from fluent.syntax import ast

from ftl_extract.matcher import FluentKey
from ftl_extract.process.serializer import BeautyFluentSerializer, generate_ftl


@pytest.fixture()
def single_fluent_key() -> list[FluentKey]:
    return [
        FluentKey(
            code_path=Path("test.py"),
            key="greeting",
            translation=ast.Message(
                id=ast.Identifier("greeting"),
                value=ast.Pattern(elements=[ast.TextElement("Hello, world!")]),
            ),
        )
    ]


@pytest.fixture()
def multiple_fluent_keys() -> list[FluentKey]:
    return [
        FluentKey(
            code_path=Path("test.py"),
            key="greeting",
            translation=ast.Message(
                id=ast.Identifier("greeting"),
                value=ast.Pattern(elements=[ast.TextElement("Hello, world!")]),
            ),
        ),
        FluentKey(
            code_path=Path("test.py"),
            key="farewell",
            translation=ast.Message(
                id=ast.Identifier("farewell"),
                value=ast.Pattern(elements=[ast.TextElement("Goodbye, world!")]),
            ),
        ),
    ]


@pytest.fixture()
def empty_fluent_keys() -> list[FluentKey]:
    return []


def test_custom_serializer_produces_correct_ftl_for_single_key(
    single_fluent_key: list[FluentKey],
) -> None:
    ftl_string, resource = generate_ftl(single_fluent_key, serializer=BeautyFluentSerializer())
    assert "greeting = Hello, world!" in ftl_string
    assert len(resource.body) == 1


def test_custom_serializer_produces_correct_ftl_for_multiple_keys(
    multiple_fluent_keys: list[FluentKey],
) -> None:
    ftl_string, resource = generate_ftl(multiple_fluent_keys, serializer=BeautyFluentSerializer())
    assert "greeting = Hello, world!" in ftl_string
    assert "farewell = Goodbye, world!" in ftl_string
    assert len(resource.body) == 2  # noqa: PLR2004


def test_custom_serializer_handles_empty_fluent_keys_list_properly(
    empty_fluent_keys: list[FluentKey],
) -> None:
    ftl_string, resource = generate_ftl(empty_fluent_keys, serializer=BeautyFluentSerializer())
    assert ftl_string == ""
    assert resource.body is None or len(resource.body) == 0
