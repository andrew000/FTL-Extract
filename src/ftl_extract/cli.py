from __future__ import annotations

from pathlib import Path
from time import perf_counter_ns
from typing import Literal

import click

from ftl_extract.const import (
    COMMENT_KEYS_MODE,
    DEFAULT_EXCLUDE_DIRS,
    DEFAULT_FTL_FILE,
    DEFAULT_I18N_KEYS,
    DEFAULT_IGNORE_ATTRIBUTES,
    DEFAULT_IGNORE_KWARGS,
)
from ftl_extract.ftl_extractor import extract
from ftl_extract.stub.generator import generate_stubs


@click.group("ftl")
@click.version_option()
def ftl() -> None: ...


@ftl.command("extract")
@click.argument("code_path", type=click.Path(exists=True, path_type=Path))
@click.argument("output_path", type=click.Path(path_type=Path))
@click.option(
    "--language",
    "-l",
    multiple=True,
    default=("en",),
    show_default=True,
    help="Language of translation.",
)
@click.option(
    "--i18n-keys",
    "-k",
    default=DEFAULT_I18N_KEYS,
    multiple=True,
    show_default=True,
    help="Names of function that is used to get translation.",
)
@click.option(
    "--i18n-keys-append",
    "-K",
    default=(),
    multiple=True,
    help="Append names of function that is used to get translation.",
)
@click.option(
    "--i18n-keys-prefix",
    "-p",
    default=(),
    multiple=True,
    help="Prefix names of function that is used to get translation. `self.i18n.*()`",
)
@click.option(
    "--exclude-dirs",
    "-e",
    multiple=True,
    default=DEFAULT_EXCLUDE_DIRS,
    show_default=True,
    help="Exclude directories.",
)
@click.option(
    "--exclude-dirs-append",
    "-E",
    default=(),
    multiple=True,
    help="Append directories to exclude.",
)
@click.option(
    "--ignore-attributes",
    "-i",
    default=DEFAULT_IGNORE_ATTRIBUTES,
    multiple=True,
    show_default=True,
    help="Ignore attributes, like `i18n.set_locale`.",
)
@click.option(
    "--append-ignore-attributes",
    "-I",
    multiple=True,
    help="Append attributes to ignore.",
)
@click.option(
    "--ignore-kwargs",
    default=DEFAULT_IGNORE_KWARGS,
    multiple=True,
    show_default=True,
    help="Ignore kwargs, like `when` from `aiogram_dialog.I18nFormat(..., when=...)`.",
)
@click.option(
    "--comment-junks",
    is_flag=True,
    default=False,
    show_default=True,
    help="Comments Junk elements.",
)
@click.option(
    "--default-ftl-file",
    default=DEFAULT_FTL_FILE,
    show_default=True,
    type=click.Path(path_type=Path),
)
@click.option(
    "--comment-keys-mode",
    default=COMMENT_KEYS_MODE[0],
    show_default=True,
    help="Comment keys mode.",
    type=click.Choice(COMMENT_KEYS_MODE, case_sensitive=False),
)
@click.option(
    "--dry-run",
    is_flag=True,
    default=False,
    show_default=True,
    help="Do not write to output files.",
)
@click.option(
    "--verbose",
    "-v",
    is_flag=True,
    default=False,
    show_default=True,
    help="Verbose output.",
)
def cli_extract(
    code_path: Path,
    output_path: Path,
    language: tuple[str, ...],
    i18n_keys: tuple[str, ...],
    i18n_keys_append: tuple[str, ...],
    i18n_keys_prefix: tuple[str, ...],
    exclude_dirs: tuple[str, ...],
    exclude_dirs_append: tuple[str, ...],
    ignore_attributes: tuple[str, ...],
    append_ignore_attributes: tuple[str, ...],
    ignore_kwargs: tuple[str, ...],
    comment_junks: bool,
    default_ftl_file: Path,
    comment_keys_mode: Literal["comment", "warn"],
    dry_run: bool,
    verbose: bool,
) -> None:
    click.echo(f"Extracting from {code_path}")
    start_time = perf_counter_ns()

    statistics = extract(
        code_path=code_path,
        output_path=output_path,
        language=language,
        i18n_keys=i18n_keys,
        i18n_keys_append=i18n_keys_append,
        i18n_keys_prefix=i18n_keys_prefix,
        exclude_dirs=exclude_dirs,
        exclude_dirs_append=exclude_dirs_append,
        ignore_attributes=ignore_attributes,
        append_ignore_attributes=append_ignore_attributes,
        ignore_kwargs=ignore_kwargs,
        comment_junks=comment_junks,
        default_ftl_file=default_ftl_file,
        comment_keys_mode=comment_keys_mode,
        dry_run=dry_run,
    )

    if verbose:
        click.echo("Extraction statistics:")
        click.echo(f"  - Py files count: {statistics.py_files_count}")
        click.echo(f"  - FTL files count: {statistics.ftl_files_count}")
        click.echo(f"  - FTL keys in code: {statistics.ftl_in_code_keys_count}")
        click.echo(f"  - FTL keys stored: {statistics.ftl_stored_keys_count}")
        click.echo(f"  - FTL keys updated: {statistics.ftl_keys_updated}")
        click.echo(f"  - FTL keys added: {statistics.ftl_keys_added}")
        click.echo(f"  - FTL keys commented: {statistics.ftl_keys_commented}")

    click.echo(f"[Python] Done in {(perf_counter_ns() - start_time) * 1e-9:.3f}s.")


@ftl.command("stub")
@click.argument("locale_path", type=click.Path(exists=True, path_type=Path))
@click.argument("output_path", type=click.Path(path_type=Path))
def cli_stub(locale_path: Path, output_path: Path) -> None:
    generate_stubs(locale_path, output_path)
