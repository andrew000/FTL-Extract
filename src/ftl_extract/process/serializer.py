from __future__ import annotations

import operator
from typing import TYPE_CHECKING

from fluent.syntax import FluentSerializer, ast
from fluent.syntax.serializer import serialize_junk, serialize_message, serialize_term

if TYPE_CHECKING:
    from collections.abc import Iterable

    from fluent.syntax.ast import Resource

    from ftl_extract.matcher import FluentKey


class BeautyFluentSerializer(FluentSerializer):
    """A serializer that formats the output FTL for better readability."""

    def serialize_entry(self, entry: ast.EntryType, state: int = 0) -> str:  # pragma: no cover
        """Serialize an :class:`.ast.Entry` to a string."""
        if isinstance(entry, ast.Message):
            return serialize_message(entry)
        if isinstance(entry, ast.Term):
            return serialize_term(entry)
        if isinstance(entry, ast.Comment):
            if state & self.HAS_ENTRIES:
                return "\n{}\n".format(serialize_comment(entry, "#"))
            return "{}\n".format(serialize_comment(entry, "#"))
        if isinstance(entry, ast.GroupComment):
            if state & self.HAS_ENTRIES:
                return "\n{}\n".format(serialize_comment(entry, "##"))
            return "{}\n".format(serialize_comment(entry, "##"))
        if isinstance(entry, ast.ResourceComment):
            if state & self.HAS_ENTRIES:
                return "\n{}\n".format(serialize_comment(entry, "###"))
            return "{}\n".format(serialize_comment(entry, "###"))
        if isinstance(entry, ast.Junk):
            return serialize_junk(entry)
        raise Exception(f"Unknown entry type: {type(entry)}")  # noqa: TRY002, TRY003, EM102


def serialize_comment(
    comment: ast.Comment | ast.GroupComment | ast.ResourceComment,
    prefix: str = "#",
) -> str:  # pragma: no cover
    if not comment.content:
        return f"{prefix}"

    return "\n".join(
        [prefix if len(line) == 0 else f"{prefix} {line}" for line in comment.content.split("\n")]
    )


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
