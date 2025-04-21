from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Final


I18N_LITERAL: Final[str] = "i18n"
GET_LITERAL: Final[str] = "get"
PATH_LITERAL: Final[str] = "_path"
IGNORE_ATTRIBUTES: Final[frozenset[str]] = frozenset(
    {"set_locale", "use_locale", "use_context", "set_context"},
)
IGNORE_KWARGS: Final[frozenset[str]] = frozenset()
DEFAULT_FTL_FILE: Final[Path] = Path("_default.ftl")
FTL_DEBUG_VAR_NAME: Final[str] = "FTL_DEBUG"
COMMENT_KEYS_MODE: tuple[str, ...] = ("comment", "warn")
