from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Final

I18N_LITERAL: Final[str] = "i18n"
GET_LITERAL: Final[str] = "get"
PATH_LITERAL: Final[str] = "_path"
DEFAULT_I18N_KEYS: Final[tuple[str, ...]] = (I18N_LITERAL, "L", "LazyProxy", "LazyFilter")
DEFAULT_IGNORE_ATTRIBUTES: Final[tuple[str, ...]] = (
    "set_locale",
    "use_locale",
    "use_context",
    "set_context",
)
DEFAULT_IGNORE_KWARGS: Final[tuple[str, ...]] = ()
DEFAULT_FTL_FILE: Final[Path] = Path("_default.ftl")
FTL_DEBUG_VAR_NAME: Final[str] = "FTL_DEBUG"
COMMENT_KEYS_MODE: Final[tuple[str, ...]] = ("comment", "warn")
DEFAULT_EXCLUDE_DIRS: Final[tuple[str, ...]] = (
    ".venv",
    "venv",
    ".git",
    "__pycache__",
    ".pytest_cache",
)
