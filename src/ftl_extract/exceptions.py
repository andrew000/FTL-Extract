from __future__ import annotations

from os import environ
from pprint import pformat
from typing import TYPE_CHECKING

from ftl_extract.const import FTL_DEBUG_VAR_NAME
from ftl_extract.utils import to_json_no_span

if TYPE_CHECKING:
    from pathlib import Path

    from fluent.syntax import ast


class FTLExtractorError(Exception):
    pass


class FTLExtractorDifferentPathsError(FTLExtractorError):
    def __init__(self, key: str, current_path: Path, new_path: Path) -> None:
        self.current_path = current_path
        self.new_path = new_path
        super().__init__(
            f"Key {key!r} already exists with different path: "
            f"{self.current_path} != {self.new_path}",
        )


class FTLExtractorDifferentTranslationError(FTLExtractorError):
    def __init__(
        self,
        key: str,
        current_translation: ast.Message,
        new_translation: ast.Message,
    ) -> None:
        self.current_translation = current_translation
        self.new_translation = new_translation

        if bool(environ.get(FTL_DEBUG_VAR_NAME, "")) is True:
            super().__init__(
                f"Translation {key!r} already exists with different elements:\n"
                f"current_translation: "
                f"{pformat(self.current_translation.to_json(fn=to_json_no_span))}\n!= "
                f"new_translation: {pformat(self.new_translation.to_json(fn=to_json_no_span))}",
            )
        else:
            super().__init__(
                f"Translation {key!r} already exists with different elements: "
                f"{self.current_translation} != {self.new_translation}",
            )


class FTLExtractorCantFindReferenceError(FTLExtractorError):
    def __init__(self, key: str, key_path: Path, reference_key: str) -> None:
        self.key = key
        self.key_path = key_path
        self.reference_key = reference_key
        super().__init__(f"Can't find reference {reference_key!r} for key {key!r} at {key_path}")


class FTLExtractorCantFindTermError(FTLExtractorError):
    def __init__(self, key: str, locale: str, key_path: Path, term_key: str) -> None:
        self.key = key
        self.locale = locale
        self.key_path = key_path
        self.term_key = term_key
        super().__init__(
            f"Can't find term {term_key!r} for key {key!r} with locale {locale!r} at: {key_path}",
        )
