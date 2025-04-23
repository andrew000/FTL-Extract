from __future__ import annotations

import ast
from dataclasses import dataclass, field
from math import inf
from pathlib import Path
from typing import TYPE_CHECKING, cast

from fluent.syntax import ast as fluent_ast

from ftl_extract.const import (
    DEFAULT_I18N_KEYS,
    DEFAULT_IGNORE_ATTRIBUTES,
    DEFAULT_IGNORE_KWARGS,
    GET_LITERAL,
    PATH_LITERAL,
)
from ftl_extract.exceptions import (
    FTLExtractorDifferentPathsError,
    FTLExtractorDifferentTranslationError,
)
from ftl_extract.utils import to_json_no_span

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
    path: Path
    locale: str | None = field(default=None)
    position: int | float = field(default=inf)
    depends_on_keys: set[str] = field(default_factory=set)

    def __repr__(self) -> str:
        return (
            f"FluentKey("
            f"code_path={self.code_path},"
            f"key={self.key},"
            f"path={self.path},"
            f"locale={self.locale},"
            f"position={self.position},"
            f"translation={self.translation.to_json(fn=to_json_no_span)}"
            f")"
        )

    def __str__(self) -> str:
        return (
            f"FluentKey(\n"
            f"\tcode_path={self.code_path},\n"
            f"\tkey={self.key},\n"
            f"\tpath={self.path},\n"
            f"\tlocale={self.locale},\n"
            f"\tposition={self.position},\n"
            f"\ttranslation={self.translation.to_json(fn=to_json_no_span)}\n"
            f")"
        )


class I18nMatcher(ast.NodeVisitor):
    def __init__(
        self,
        code_path: Path,
        default_ftl_file: Path,
        i18n_keys: Iterable[str] = DEFAULT_I18N_KEYS,
        ignore_attributes: Iterable[str] = DEFAULT_IGNORE_ATTRIBUTES,
        ignore_kwargs: Iterable[str] = DEFAULT_IGNORE_KWARGS,
    ) -> None:
        """

        :param code_path: Path to .py file where visitor will be used.
        :type code_path: Path
        :param default_ftl_file: Default name of FTL file.
        :type default_ftl_file: Path
        :param i18n_keys: Name of function that is used to get translation. Default is ("i18n",).
        :type i18n_keys: Iterable[str]
        :param ignore_attributes: Ignore attributes, like `i18n.set_locale`.
        :type ignore_attributes: Iterable[str]
        :param ignore_kwargs: Ignore kwargs, like `when` from
        `aiogram_dialog.I18nFormat(..., when=...)`.
        :type ignore_kwargs: Iterable[str]
        """
        self.code_path = code_path
        self.i18n_keys = frozenset(i18n_keys)
        self.ignore_attributes = frozenset(ignore_attributes)
        self.ignore_kwargs = frozenset(ignore_kwargs)
        self.default_ftl_file = default_ftl_file
        self.fluent_keys: dict[str, FluentKey] = {}

    def visit_Call(self, node: ast.Call) -> None:  # noqa: N802
        # Check if the call matches the pattern
        if isinstance(node.func, ast.Attribute):
            attr: ast.Attribute | ast.expr = node.func
            attrs = []
            while isinstance(attr, ast.Attribute):
                attrs.append(attr.attr)
                attr = attr.value

            if isinstance(attr, ast.Name) and attr.id in self.i18n_keys:
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
                        ignore_kwargs=self.ignore_kwargs,
                        default_ftl_file=self.default_ftl_file,
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
                        ignore_kwargs=self.ignore_kwargs,
                        default_ftl_file=self.default_ftl_file,
                    )
                    process_fluent_key(self.fluent_keys, fluent_key)
            else:
                self.generic_visit(node)

        elif isinstance(node.func, ast.Name) and node.func.id in self.i18n_keys:
            if not node.args or not isinstance(node.args[0], ast.Constant):
                return

            fluent_key = create_fluent_key(
                code_path=self.code_path,
                key=cast(ast.Constant, node.args[0]).value,
                keywords=node.keywords,
                ignore_kwargs=self.ignore_kwargs,
                default_ftl_file=self.default_ftl_file,
            )
            process_fluent_key(self.fluent_keys, fluent_key)

        self.generic_visit(node)


def create_fluent_key(
    code_path: Path,
    key: str,
    keywords: list[ast.keyword],
    ignore_kwargs: frozenset[str],
    default_ftl_file: Path,
) -> FluentKey:
    fluent_key = FluentKey(
        code_path=code_path,
        key=key,
        translation=fluent_ast.Message(
            id=fluent_ast.Identifier(name=key),
            value=fluent_ast.Pattern(elements=[fluent_ast.TextElement(value=key)]),
        ),
        path=default_ftl_file,
    )

    keywords = sorted(
        keywords,
        key=lambda keyword: keyword.arg or "",
    )

    for kw in keywords:
        if kw.arg == PATH_LITERAL:
            if kw.value is not None and isinstance(kw.value, ast.Constant):
                fluent_key.path = Path(cast(ast.Constant, kw.value).value)
        elif isinstance(kw.arg, str):
            if kw.arg in ignore_kwargs:
                continue

            cast(
                fluent_ast.Pattern,
                cast(fluent_ast.Message, fluent_key.translation).value,
            ).elements.append(
                fluent_ast.Placeable(
                    expression=fluent_ast.VariableReference(id=fluent_ast.Identifier(name=kw.arg)),
                ),
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
