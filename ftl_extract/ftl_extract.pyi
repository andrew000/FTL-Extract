from __future__ import annotations

from pathlib import Path
from typing import Literal

def fast_extract(
        code_path: Path,
        output_path: Path,
        language: tuple[str, ...],
        i18n_keys: set[str],
        i18n_keys_append: set[str],
        i18n_keys_prefix: set[str],
        exclude_dirs: set[str],
        exclude_dirs_append: set[str],
        ignore_attributes: set[str],
        append_ignore_attributes: set[str],
        ignore_kwargs: set[str],
        comment_junks: bool,
        default_ftl_file: Path,
        comment_keys_mode: Literal["comment", "warn"],
        dry_run: bool,
) -> None:
    ...
