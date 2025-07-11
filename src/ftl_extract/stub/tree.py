from __future__ import annotations

from typing import TYPE_CHECKING, Any, Literal, TypedDict, cast

from fluent.syntax.serializer import serialize_pattern

if TYPE_CHECKING:
    from ftl_extract.stub.visitor import Message

METADATA_DICT_KEY: Literal["$_meta$"] = "$_meta$"


class Metadata(TypedDict, total=False):
    args: list[str]
    translation: str


def generate_tree(fluent_messages: dict[str, Message]) -> dict[str, dict[str, Any]]:
    tree: dict[str, dict[str, Any]] = {}

    for key, message in fluent_messages.items():
        key_parts = key.split("-")
        key_parts_len = len(key_parts)

        inner_tree = tree
        for index, key_part in enumerate(key_parts, start=1):
            if index == key_parts_len:
                if key_part in inner_tree:
                    cast(dict[str, Any], inner_tree[key_part])[METADATA_DICT_KEY] = Metadata(
                        args=message.kwargs,
                        translation=serialize_pattern(message.fluent_message.value).strip(),
                    )
                else:
                    inner_tree.setdefault(
                        key_part,
                        {
                            METADATA_DICT_KEY: Metadata(
                                args=message.kwargs,
                                translation=serialize_pattern(message.fluent_message.value).strip(),
                            ),
                        },
                    )
            else:
                inner_tree = inner_tree.setdefault(key_part, {})

    return tree
