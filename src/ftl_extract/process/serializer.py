from __future__ import annotations

import operator
from typing import TYPE_CHECKING

from fluent.syntax import FluentSerializer, ast

if TYPE_CHECKING:
    from collections.abc import Iterable

    from fluent.syntax.ast import Resource

    from ftl_extract.matcher import FluentKey


def generate_ftl(
    fluent_keys: Iterable[FluentKey],
    serializer: FluentSerializer,
    leave_as_is: Iterable[FluentKey],
) -> tuple[str, Resource]:
    """Generate FTL translations from `fluent_keys`."""
    resource = ast.Resource(body=None)

    listed_fluent_keys = list(fluent_keys)
    listed_fluent_keys.extend(leave_as_is)

    # Sort fluent keys by position
    listed_fluent_keys.sort(key=operator.attrgetter("position"))

    for fluent_key in listed_fluent_keys:
        resource.body.append(fluent_key.translation)

    return serializer.serialize(resource), resource
