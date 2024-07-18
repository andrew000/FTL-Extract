from pathlib import Path

from fluent.syntax import ast

from ftl_extract.matcher import FluentKey
from ftl_extract.process.kwargs_extractor import extract_kwargs


def extracts_variable_names_from_simple_variable() -> None:
    kwargs = extract_kwargs(
        FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Message(
                id=ast.Identifier("test_message"),
                value=ast.Pattern(
                    elements=[
                        ast.TextElement("Hello, "),
                        ast.Placeable(
                            expression=ast.VariableReference(id=ast.Identifier("username"))
                        ),
                    ]
                ),
            ),
        )
    )
    assert kwargs == {"username"}


def extracts_variable_names_from_select_expression() -> None:
    kwargs = extract_kwargs(
        FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Message(
                id=ast.Identifier("select_message"),
                value=ast.Pattern(
                    elements=[
                        ast.Placeable(
                            expression=ast.SelectExpression(
                                selector=ast.VariableReference(id=ast.Identifier("gender")),
                                variants=[
                                    ast.Variant(
                                        key=ast.Identifier("male"),
                                        value=ast.Pattern(elements=[ast.TextElement("Mr.")]),
                                        default=False,
                                    ),
                                    ast.Variant(
                                        key=ast.Identifier("female"),
                                        value=ast.Pattern(elements=[ast.TextElement("Ms.")]),
                                        default=False,
                                    ),
                                ],
                            )
                        )
                    ]
                ),
            ),
        )
    )
    assert kwargs == {"gender"}


def returns_empty_set_for_messages_without_variables() -> None:
    kwargs = extract_kwargs(
        FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Message(
                id=ast.Identifier("no_variables"),
                value=ast.Pattern(elements=[ast.TextElement("Just a text message.")]),
            ),
        )
    )
    assert kwargs == set()


def test_returns_empty_set_for_comment_translation() -> None:
    kwargs = extract_kwargs(
        FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Comment(content="This is a comment"),
        )
    )
    assert kwargs == set()


def test_returns_empty_set_for_translation_without_value() -> None:
    kwargs = extract_kwargs(
        FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Message(
                id=ast.Identifier("message_no_value"),
                value=None,  # Explicitly setting value to None
            ),
        )
    )
    assert kwargs == set()


def test_extracts_variable_names_from_mixed_elements() -> None:
    kwargs = extract_kwargs(
        FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Message(
                id=ast.Identifier("mixed_message"),
                value=ast.Pattern(
                    elements=[
                        ast.TextElement("Hello, "),
                        ast.Placeable(
                            expression=ast.VariableReference(id=ast.Identifier("username"))
                        ),
                        ast.TextElement(" your balance is "),
                        ast.Placeable(
                            expression=ast.VariableReference(id=ast.Identifier("balance"))
                        ),
                    ]
                ),
            ),
        )
    )
    assert kwargs == {"username", "balance"}


def test_extracts_selector_variable_name_from_select_expression() -> None:
    kwargs = extract_kwargs(
        FluentKey(
            code_path=Path("test.py"),
            key="key_select_expression",
            translation=ast.Message(
                id=ast.Identifier("select_expression_message"),
                value=ast.Pattern(
                    elements=[
                        ast.Placeable(
                            expression=ast.SelectExpression(
                                selector=ast.VariableReference(id=ast.Identifier("user_status")),
                                variants=[
                                    ast.Variant(
                                        key=ast.Identifier("active"),
                                        value=ast.Pattern(
                                            elements=[ast.TextElement("Active User")]
                                        ),
                                        default=False,
                                    ),
                                    ast.Variant(
                                        key=ast.Identifier("inactive"),
                                        value=ast.Pattern(
                                            elements=[ast.TextElement("Inactive User")]
                                        ),
                                        default=False,
                                    ),
                                ],
                            )
                        )
                    ]
                ),
            ),
        )
    )
    assert kwargs == {"user_status"}
