from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


def to_json_no_span(value: dict[str, Any]) -> Any:
    value.pop("span", None)
    return value


@dataclass
class ExtractionStatistics:
    py_files_count: int = field(default=0)
    ftl_files_count: dict[str, int] = field(default_factory=dict)  # dict[lang, count]
    ftl_in_code_keys_count: int = field(default=0)
    ftl_stored_keys_count: dict[str, int] = field(default_factory=dict)  # dict[lang, count]
    ftl_keys_updated: dict[str, int] = field(default_factory=dict)  # dict[lang, count]
    ftl_keys_added: dict[str, int] = field(default_factory=dict)  # dict[lang, count]
    ftl_keys_commented: dict[str, int] = field(default_factory=dict)  # dict[lang, count]


def prepare_exclude_dirs(
    *,
    exclude_dirs: tuple[str, ...],
    exclude_dirs_append: tuple[str, ...],
) -> frozenset[Path]:
    return frozenset(
        [Path(exclude_dir) for exclude_dir in exclude_dirs + exclude_dirs_append],
    )
