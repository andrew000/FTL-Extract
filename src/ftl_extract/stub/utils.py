from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Sequence
    from typing import Any


def to_camel_case(string: str) -> str:
    return "".join(x.capitalize() for x in string.lower().split("_"))


def remove_duplicates(sequence: Sequence[Any]) -> list[Any]:
    exists_set = set()
    return [x for x in sequence if not (x in exists_set or exists_set.add(x))]  # type: ignore[func-returns-value]
