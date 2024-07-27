from __future__ import annotations

import ast
from dataclasses import dataclass, field
from math import inf
from pathlib import Path
from typing import TYPE_CHECKING, cast

from fluent.syntax import ast as fluent_ast

from ftl_extract.const import GET_LITERAL, I18N_LITERAL, IGNORE_ATTRIBUTES, PATH_LITERAL
from ftl_extract.exceptions import (
    FTLExtractorDifferentPathsError,
    FTLExtractorDifferentTranslationError,
)

if TYPE_CHECKING:
    from collections.abc import Iterable


@dataclass
class FluentKey:
    """
    Dataclass for storing information about key and its translation.

    :param code_path: Path to .py file where key was found.
    :type code_path: Path
    :param key: Key that will be used to get translation.
    :type key: str | None
    :param translation: Translation of key.
    :type translation: str | None
    :param path: Path to .ftl file where key will be stored.
    :type path: Path
    :param locale: Locale of translation. When extracting from .py file, it will not be needed.
    :type locale: str | None
    """

    code_path: Path
    key: str
    translation: fluent_ast.EntryType
    path: Path = field(default=Path("_default.ftl"))
    locale: str | None = field(default=None)
    position: int | float = field(default=inf)


class I18nMatcher(ast.NodeVisitor):
    def __init__(
        self,
        code_path: Path,
        func_names: str | Iterable[str] = I18N_LITERAL,
        ignore_attributes: str | Iterable[str] = IGNORE_ATTRIBUTES,
    ) -> None:
        """

        :param code_path: Path to .py file where visitor will be used.
        :type code_path: Path
        :param func_names: Name of function that is used to get translation. Default is "i18n".
        :type func_names: str | Sequence[str]
        """
        self.code_path = code_path
        self.func_names = (
            frozenset({func_names}) if isinstance(func_names, str) else frozenset(func_names)
        )
        self.ignore_attributes = (
            frozenset({ignore_attributes})
            if isinstance(ignore_attributes, str)
            else frozenset(ignore_attributes)
        )
        self.fluent_keys: dict[str, FluentKey] = {}

    def visit_Call(self, node: ast.Call) -> None:  # noqa: N802
        # Check if the call matches the pattern
        if isinstance(node.func, ast.Attribute):
            attr: ast.Attribute | ast.expr = node.func
            attrs = []
            while isinstance(attr, ast.Attribute):
                attrs.append(attr.attr)
                attr = attr.value

            if isinstance(attr, ast.Name) and attr.id in self.func_names:
                if len(attrs) == 1 and attrs[0] == GET_LITERAL:
                    # Check if the call has args
                    if not node.args:
                        return  # Skip if no args

                    # Add the first arg as the translation key
                    attrs.clear()
                    if isinstance(arg := node.args[0], ast.Constant):
                        key = cast(ast.Constant, arg).value

                    else:
                        self.generic_visit(node)
                        return

                    fluent_key = create_fluent_key(
                        code_path=self.code_path,
                        key=key,
                        keywords=node.keywords,
                    )

                    process_fluent_key(self.fluent_keys, fluent_key)

                else:
                    if attrs[-1] in self.ignore_attributes:
                        self.generic_visit(node)
                        return

                    fluent_key = create_fluent_key(
                        code_path=self.code_path,
                        key="-".join(reversed(attrs)),
                        keywords=node.keywords,
                    )
                    process_fluent_key(self.fluent_keys, fluent_key)
            else:
                self.generic_visit(node)

        elif isinstance(node.func, ast.Name) and node.func.id in self.func_names:
            if not node.args:
                return

            fluent_key = create_fluent_key(
                code_path=self.code_path,
                key=cast(ast.Constant, node.args[0]).value,
                keywords=node.keywords,
            )
            process_fluent_key(self.fluent_keys, fluent_key)

        self.generic_visit(node)


def create_fluent_key(
    code_path: Path,
    key: str,
    keywords: list[ast.keyword],
) -> FluentKey:
    fluent_key = FluentKey(
        code_path=code_path,
        key=key,
        translation=fluent_ast.Message(
            id=fluent_ast.Identifier(name=key),
            value=fluent_ast.Pattern(elements=[fluent_ast.TextElement(value=key)]),
        ),
    )

    for kw in keywords:
        if kw.arg == PATH_LITERAL:
            if kw.value is not None and isinstance(kw.value, ast.Constant):
                fluent_key.path = Path(kw.value.value)
        elif isinstance(kw.arg, str):
            cast(
                fluent_ast.Pattern, cast(fluent_ast.Message, fluent_key.translation).value
            ).elements.append(
                fluent_ast.Placeable(
                    expression=fluent_ast.VariableReference(id=fluent_ast.Identifier(name=kw.arg))
                )
            )

    return fluent_key


def process_fluent_key(fluent_keys: dict[str, FluentKey], new_fluent_key: FluentKey) -> None:
    if new_fluent_key.key in fluent_keys:
        if fluent_keys[new_fluent_key.key].path != new_fluent_key.path:
            raise FTLExtractorDifferentPathsError(
                new_fluent_key.key,
                new_fluent_key.path,
                fluent_keys[new_fluent_key.key].path,
            )
        if not fluent_keys[new_fluent_key.key].translation.equals(new_fluent_key.translation):
            raise FTLExtractorDifferentTranslationError(
                new_fluent_key.key,
                cast(fluent_ast.Message, new_fluent_key.translation),
                cast(fluent_ast.Message, fluent_keys[new_fluent_key.key].translation),
            )

    else:
        fluent_keys[new_fluent_key.key] = new_fluent_key
