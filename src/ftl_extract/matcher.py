from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, field
from math import inf
from pathlib import Path
from typing import TYPE_CHECKING, Callable, cast

import libcst as cst
from fluent.syntax import ast
from libcst import matchers as m

from ftl_extract.exceptions import (
    FTLExtractorDifferentPathsError,
    FTLExtractorDifferentTranslationError,
)

if TYPE_CHECKING:
    from typing import Literal

I18N_LITERAL: Literal["i18n"] = "i18n"
GET_LITERAL: Literal["get"] = "get"
PATH_LITERAL: Literal["_path"] = "_path"


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
    translation: (
        ast.Message | ast.Comment | ast.Term | ast.GroupComment | ast.ResourceComment | ast.Junk
    )
    path: Path = field(default=Path("_default.ftl"))
    locale: str | None = field(default=None)
    position: int | float = field(default=inf)


class I18nMatcher:
    def __init__(self, code_path: Path, func_names: str | Sequence[str] = I18N_LITERAL) -> None:
        """

        :param code_path: Path to .py file where visitor will be used.
        :type code_path: Path
        :param func_names: Name of function that is used to get translation. Default is "i18n".
        :type func_names: str | Sequence[str]
        """
        self.code_path = code_path
        self._func_names = {func_names} if isinstance(func_names, str) else set(func_names)
        self.fluent_keys: dict[str, FluentKey] = {}

        self._matcher = m.OneOf(
            m.Call(
                func=m.Attribute(
                    value=m.OneOf(*map(cast(Callable, m.Name), self._func_names)),
                    attr=m.SaveMatchedNode(matcher=~m.Name(GET_LITERAL) & m.Name(), name="key"),
                ),
                args=[
                    m.SaveMatchedNode(
                        matcher=m.ZeroOrMore(
                            m.Arg(
                                value=m.DoNotCare(),
                                keyword=m.Name(),
                            )
                        ),
                        name="kwargs",
                    )
                ],
            ),
            m.Call(
                func=m.Attribute(
                    value=m.OneOf(*map(cast(Callable, m.Name), self._func_names)),
                    attr=m.Name(value=GET_LITERAL),
                ),
                args=[
                    m.Arg(
                        value=m.SaveMatchedNode(matcher=m.SimpleString(), name="key"), keyword=None
                    ),
                    m.SaveMatchedNode(
                        matcher=m.ZeroOrMore(
                            m.Arg(
                                value=m.DoNotCare(),
                                keyword=m.Name(),
                            )
                        ),
                        name="kwargs",
                    ),
                ],
            ),
            m.Call(
                func=m.OneOf(*map(cast(Callable, m.Name), self._func_names)),
                args=[
                    m.Arg(
                        value=m.SaveMatchedNode(matcher=m.SimpleString(), name="key"), keyword=None
                    ),
                    m.SaveMatchedNode(
                        matcher=m.ZeroOrMore(
                            m.Arg(
                                value=m.DoNotCare(),
                                keyword=m.Name(),
                            )
                        ),
                        name="kwargs",
                    ),
                ],
            ),
        )

    def extract_matches(self, module: cst.Module) -> None:
        for match in m.extractall(module, self._matcher):
            # Key
            if isinstance(match["key"], cst.Name):
                key = cast(cst.Name, match["key"]).value
                translation = ast.Message(
                    id=ast.Identifier(name=key),
                    value=ast.Pattern(
                        elements=[ast.TextElement(value=cast(cst.Name, match["key"]).value)]
                    ),
                )
                fluent_key = FluentKey(
                    code_path=self.code_path,
                    key=key,
                    translation=translation,
                )
            elif isinstance(match["key"], cst.SimpleString):
                key = cast(cst.SimpleString, match["key"]).raw_value
                translation = ast.Message(
                    id=ast.Identifier(name=key),
                    value=ast.Pattern(elements=[ast.TextElement(value=key)]),
                )
                fluent_key = FluentKey(
                    code_path=self.code_path,
                    key=key,
                    translation=translation,
                )
            else:
                msg = f"Unknown type of key: {type(match['key'])} | {match['key']}"
                raise TypeError(msg)

            # Kwargs
            for kwarg in cast(Sequence[m.Arg], match["kwargs"]):
                keyword = cast(cst.Name, kwarg.keyword)
                if keyword.value == PATH_LITERAL:
                    fluent_key.path = Path(cast(cst.SimpleString, kwarg.value).raw_value)

                else:
                    if (
                        isinstance(fluent_key.translation, ast.Message)
                        and fluent_key.translation.value is not None
                    ):
                        fluent_key.translation.value.elements.append(
                            ast.Placeable(
                                expression=ast.VariableReference(
                                    id=ast.Identifier(name=keyword.value)
                                )
                            )
                        )

            if fluent_key.key in self.fluent_keys:
                if self.fluent_keys[fluent_key.key].path != fluent_key.path:
                    raise FTLExtractorDifferentPathsError(
                        fluent_key.key,
                        fluent_key.path,
                        self.fluent_keys[fluent_key.key].path,
                    )

                if not self.fluent_keys[fluent_key.key].translation.equals(fluent_key.translation):
                    raise FTLExtractorDifferentTranslationError(
                        fluent_key.key,
                        cast(ast.Message, fluent_key.translation),
                        cast(ast.Message, self.fluent_keys[fluent_key.key].translation),
                    )

            else:
                self.fluent_keys[fluent_key.key] = fluent_key
