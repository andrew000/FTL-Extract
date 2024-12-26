from pathlib import Path

import pytest
from fluent.syntax import ast

from ftl_extract.const import DEFAULT_FTL_FILE
from ftl_extract.exceptions import FTLExtractorCantFindReferenceError, FTLExtractorCantFindTermError
from ftl_extract.matcher import FluentKey
from ftl_extract.process.kwargs_extractor import extract_kwargs


def extracts_variable_names_from_simple_variable() -> None:
    kwargs = extract_kwargs(
        key=FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Message(
                id=ast.Identifier("test_message"),
                value=ast.Pattern(
                    elements=[
                        ast.TextElement("Hello, "),
                        ast.Placeable(
                            expression=ast.VariableReference(id=ast.Identifier("username")),
                        ),
                    ],
                ),
            ),
            path=DEFAULT_FTL_FILE,
        ),
    )
    assert kwargs == {"username"}


def extracts_variable_names_from_select_expression() -> None:
    kwargs = extract_kwargs(
        key=FluentKey(
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
                            ),
                        ),
                    ],
                ),
            ),
            path=DEFAULT_FTL_FILE,
        ),
    )
    assert kwargs == {"gender"}


def returns_empty_set_for_messages_without_variables() -> None:
    kwargs = extract_kwargs(
        key=FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Message(
                id=ast.Identifier("no_variables"),
                value=ast.Pattern(elements=[ast.TextElement("Just a text message.")]),
            ),
            path=DEFAULT_FTL_FILE,
        ),
    )
    assert kwargs == set()


def test_returns_empty_set_for_comment_translation() -> None:
    kwargs = extract_kwargs(
        key=FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Comment(content="This is a comment"),
            path=DEFAULT_FTL_FILE,
        ),
    )
    assert kwargs == set()


def test_returns_empty_set_for_translation_without_value() -> None:
    kwargs = extract_kwargs(
        key=FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Message(
                id=ast.Identifier("message_no_value"),
                value=None,  # Explicitly setting value to None
            ),
            path=DEFAULT_FTL_FILE,
        ),
    )
    assert kwargs == set()


def test_extracts_variable_names_from_mixed_elements() -> None:
    kwargs = extract_kwargs(
        key=FluentKey(
            code_path=Path("test.py"),
            key="key-1",
            translation=ast.Message(
                id=ast.Identifier("mixed_message"),
                value=ast.Pattern(
                    elements=[
                        ast.TextElement("Hello, "),
                        ast.Placeable(
                            expression=ast.VariableReference(id=ast.Identifier("username")),
                        ),
                        ast.TextElement(" your balance is "),
                        ast.Placeable(
                            expression=ast.VariableReference(id=ast.Identifier("balance")),
                        ),
                    ],
                ),
            ),
            path=DEFAULT_FTL_FILE,
        ),
    )
    assert kwargs == {"username", "balance"}


def test_extracts_selector_variable_name_from_select_expression() -> None:
    kwargs = extract_kwargs(
        key=FluentKey(
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
                                            elements=[ast.TextElement("Active User")],
                                        ),
                                        default=False,
                                    ),
                                    ast.Variant(
                                        key=ast.Identifier("inactive"),
                                        value=ast.Pattern(
                                            elements=[ast.TextElement("Inactive User")],
                                        ),
                                        default=False,
                                    ),
                                ],
                            ),
                        ),
                    ],
                ),
            ),
            path=DEFAULT_FTL_FILE,
        ),
    )
    assert kwargs == {"user_status"}


def test_nested_extraction() -> None:
    kwargs = extract_kwargs(
        key=FluentKey(
            code_path=Path("test.py"),
            key="trade-waiting_for_answer",
            translation=ast.Message(
                id=ast.Identifier(name="nested-key"),
                value=ast.Pattern(
                    elements=[
                        ast.TextElement(value="nested-key\n"),
                        ast.Placeable(
                            expression=ast.SelectExpression(
                                selector=ast.VariableReference(
                                    id=ast.Identifier(name="first_level_key"),
                                ),
                                variants=[
                                    ast.Variant(
                                        key=ast.NumberLiteral(value="1"),
                                        value=ast.Pattern(
                                            elements=[
                                                ast.TextElement(value="✅ "),
                                                ast.Placeable(
                                                    expression=ast.SelectExpression(
                                                        selector=ast.VariableReference(
                                                            id=ast.Identifier(
                                                                name="second_level_key",
                                                            ),
                                                        ),
                                                        variants=[
                                                            ast.Variant(
                                                                key=ast.NumberLiteral(value="1"),
                                                                value=ast.Pattern(
                                                                    elements=[
                                                                        ast.TextElement(value="OK"),
                                                                    ],
                                                                ),
                                                            ),
                                                            ast.Variant(
                                                                key=ast.NumberLiteral(value="2"),
                                                                value=ast.Pattern(
                                                                    elements=[
                                                                        ast.TextElement(value="NO"),
                                                                    ],
                                                                ),
                                                            ),
                                                            ast.Variant(
                                                                key=ast.Identifier(name="other"),
                                                                value=ast.Pattern(
                                                                    elements=[
                                                                        ast.TextElement(
                                                                            value="ANOTHER",
                                                                        ),
                                                                    ],
                                                                ),
                                                                default=True,
                                                            ),
                                                        ],
                                                    ),
                                                ),
                                            ],
                                        ),
                                    ),
                                    ast.Variant(
                                        key=ast.NumberLiteral(value="0"),
                                        value=ast.Pattern(
                                            elements=[
                                                ast.TextElement(value="❌ "),
                                                ast.Placeable(
                                                    expression=ast.SelectExpression(
                                                        selector=ast.VariableReference(
                                                            id=ast.Identifier(
                                                                name="second_level_key",
                                                            ),
                                                        ),
                                                        variants=[
                                                            ast.Variant(
                                                                key=ast.NumberLiteral(value="1"),
                                                                value=ast.Pattern(
                                                                    elements=[
                                                                        ast.TextElement(value="OK"),
                                                                    ],
                                                                ),
                                                            ),
                                                            ast.Variant(
                                                                key=ast.NumberLiteral(value="2"),
                                                                value=ast.Pattern(
                                                                    elements=[
                                                                        ast.TextElement(value="NO"),
                                                                    ],
                                                                ),
                                                            ),
                                                            ast.Variant(
                                                                key=ast.Identifier(name="other"),
                                                                value=ast.Pattern(
                                                                    elements=[
                                                                        ast.TextElement(
                                                                            value="ANOTHER",
                                                                        ),
                                                                    ],
                                                                ),
                                                                default=True,
                                                            ),
                                                        ],
                                                    ),
                                                ),
                                            ],
                                        ),
                                    ),
                                    ast.Variant(
                                        key=ast.Identifier(name="other"),
                                        value=ast.Pattern(
                                            elements=[
                                                ast.TextElement(value="⏳ "),
                                                ast.Placeable(
                                                    expression=ast.VariableReference(
                                                        id=ast.Identifier(name="second_level_key"),
                                                    ),
                                                ),
                                                ast.TextElement(value=" ANOTHER"),
                                            ],
                                        ),
                                        default=True,
                                    ),
                                ],
                            ),
                        ),
                    ],
                ),
            ),
            path=DEFAULT_FTL_FILE,
            locale="en",
            position=0,
        ),
    )

    assert kwargs == {"first_level_key", "second_level_key"}


def test_extract_kwargs_from_message_reference() -> None:
    key = FluentKey(
        code_path=Path("test.py"),
        key="key-1",
        translation=ast.Message(
            id=ast.Identifier("test_message"),
            value=ast.Pattern(
                elements=[
                    ast.Placeable(
                        expression=ast.MessageReference(id=ast.Identifier("referenced_message")),
                    ),
                ],
            ),
        ),
        path=DEFAULT_FTL_FILE,
    )

    referenced_key = FluentKey(
        code_path=Path("test.py"),
        key="referenced_message",
        translation=ast.Message(
            id=ast.Identifier("referenced_message"),
            value=ast.Pattern(
                elements=[
                    ast.TextElement("Referenced message content"),
                ],
            ),
        ),
        path=DEFAULT_FTL_FILE,
    )

    all_fluent_keys = {"referenced_message": referenced_key}
    kwargs = extract_kwargs(key=key, all_fluent_keys=all_fluent_keys)

    assert kwargs == set()


def test_raises_error_for_missing_reference_key() -> None:
    key = FluentKey(
        code_path=Path("test.py"),
        key="key-1",
        translation=ast.Message(
            id=ast.Identifier("test_message"),
            value=ast.Pattern(
                elements=[
                    ast.Placeable(
                        expression=ast.MessageReference(id=ast.Identifier("missing_reference")),
                    ),
                ],
            ),
        ),
        path=DEFAULT_FTL_FILE,
    )

    all_fluent_keys = {}  # Empty dictionary to simulate missing reference

    with pytest.raises(FTLExtractorCantFindReferenceError):
        extract_kwargs(key=key, all_fluent_keys=all_fluent_keys)


def test_extract_kwargs_from_term_reference() -> None:
    key = FluentKey(
        code_path=Path("test.py"),
        key="key-1",
        translation=ast.Message(
            id=ast.Identifier("test_message"),
            value=ast.Pattern(
                elements=[
                    ast.Placeable(
                        expression=ast.TermReference(id=ast.Identifier("referenced_term")),
                    ),
                ],
            ),
        ),
        path=DEFAULT_FTL_FILE,
    )

    referenced_term = FluentKey(
        code_path=Path("test.py"),
        key="referenced_term",
        translation=ast.Term(
            id=ast.Identifier("referenced_term"),
            value=ast.Pattern(
                elements=[
                    ast.TextElement("Referenced term content"),
                ],
            ),
        ),
        path=DEFAULT_FTL_FILE,
    )

    terms = {"referenced_term": referenced_term}
    kwargs = extract_kwargs(key=key, terms=terms)

    assert kwargs == set()


def test_raises_error_for_missing_term() -> None:
    key = FluentKey(
        code_path=Path("test.py"),
        key="key-1",
        translation=ast.Message(
            id=ast.Identifier("test_message"),
            value=ast.Pattern(
                elements=[
                    ast.Placeable(
                        expression=ast.TermReference(id=ast.Identifier("missing_term")),
                    ),
                ],
            ),
        ),
        path=DEFAULT_FTL_FILE,
    )

    terms = {}  # Empty dictionary to simulate missing term

    with pytest.raises(FTLExtractorCantFindTermError):
        extract_kwargs(key=key, terms=terms)
