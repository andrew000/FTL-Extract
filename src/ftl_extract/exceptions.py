from __future__ import annotations

from typing import TYPE_CHECKING

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
            f"{self.current_path} != {self.new_path}"
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
        super().__init__(
            f"Translation {key!r} already exists with different elements: "
            f"{self.current_translation} != {self.new_translation}"
        )
