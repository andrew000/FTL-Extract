from typing import Any


def to_json_no_span(value: dict[str, Any]) -> Any:
    value.pop("span", None)
    return value
