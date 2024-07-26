from __future__ import annotations

from typing import Literal

I18N_LITERAL: Literal["i18n"] = "i18n"
GET_LITERAL: Literal["get"] = "get"
PATH_LITERAL: Literal["_path"] = "_path"
IGNORE_ATTRIBUTES: frozenset[str] = frozenset(
    {"set_locale", "use_locale", "use_context", "set_context"}
)
