from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING

from ftl_extract.stub.utils import remove_duplicates

if TYPE_CHECKING:
    from typing import Any

    from typing_extensions import Self

    from ftl_extract.stub.visitor import Message


@dataclass
class Node:
    name: str
    attributes: list[Self] = field(default_factory=list)
    args: list[str] = field(default_factory=list)


def fill_node(node: Node, key: str, args: list[str]) -> None:
    key_part, *other = key.split("-")

    inner_node = Node(name=key_part)

    if other:
        fill_node(inner_node, "-".join(other), args)

    else:
        inner_node.args = remove_duplicates(args)

    node.attributes.append(inner_node)


def create_node(fluent_messages: dict[str, Message]) -> Any:
    node = Node(name="i18n_stub")

    for key, message in fluent_messages.items():
        fill_node(node, key, message.kwargs)

    return node
