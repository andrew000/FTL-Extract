from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Final, Literal


I18N_LITERAL: Final[Literal["i18n"]] = "i18n"
GET_LITERAL: Final[Literal["get"]] = "get"
PATH_LITERAL: Final[Literal["_path"]] = "_path"
IGNORE_ATTRIBUTES: Final[frozenset[str]] = frozenset(
    {"set_locale", "use_locale", "use_context", "set_context"},
)
DEFAULT_FTL_FILE: Final[Path] = Path("_default.ftl")
