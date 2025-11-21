from __future__ import annotations

import subprocess
import sys
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
    LINE_ENDINGS,
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
    "--line-endings",
    default=LINE_ENDINGS[0],
    show_default=True,
    type=click.Choice(LINE_ENDINGS, case_sensitive=False),
    help="Line endings for generated FTL files.",
)
@click.option(
    "--fast",
    is_flag=True,
    default=False,
    show_default=True,
    help="Run the fast Rust implementation.",
)
@click.option(
    "--dry-run",
    is_flag=True,
    default=False,
    show_default=True,
    help="Do not write to output files.",
)
@click.option(
    "--silent",
    is_flag=True,
    default=False,
    show_default=True,
    help="Silence output files.",
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
    line_endings: Literal["default", "lf", "cr", "crlf"],
    fast: bool,
    dry_run: bool,
    silent: bool,
    verbose: bool,
) -> None:
    if fast:
        cmd = ["fast-ftl", str(code_path), str(output_path)]

        # Add multi-value options to the command
        multi_value_options = {
            "--language": language,
            "--i18n-keys": i18n_keys,
            "--i18n-keys-append": i18n_keys_append,
            "--i18n-keys-prefix": i18n_keys_prefix,
            "--exclude-dirs": exclude_dirs,
            "--exclude-dirs-append": exclude_dirs_append,
            "--ignore-attributes": ignore_attributes,
            "--append-ignore-attributes": append_ignore_attributes,
            "--ignore-kwargs": ignore_kwargs,
        }
        for option, values in multi_value_options.items():
            for value in values:
                cmd.extend([option, value])

        # Add single-value options to the command
        cmd.extend(
            [
                "--default-ftl-file",
                str(default_ftl_file),
                "--comment-keys-mode",
                comment_keys_mode,
                "--line-endings",
                line_endings,
            ]
        )

        # Boolean flags
        for flag, condition in {
            "--comment-junks": comment_junks,
            "--dry-run": dry_run,
            "--silent": silent,
            "--verbose": verbose,
        }.items():
            if condition:
                cmd.append(flag)

        if not silent:
            click.echo(f"Running fast implementation: {' '.join(cmd)}")
        try:
            result = subprocess.run(cmd, check=True)  # noqa: S603
        except FileNotFoundError:
            click.secho("Error: 'fast-ftl-extract' executable not found.", fg="red")
            click.secho(
                "Please ensure the Rust implementation is built and in your PATH.", fg="red"
            )
            sys.exit(1)
        except subprocess.CalledProcessError as e:
            # The Rust process failed. Its error message was already printed.
            # We exit with the same error code.
            sys.exit(e.returncode)
        else:
            sys.exit(result.returncode)

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
        line_endings=line_endings,
        dry_run=dry_run,
        silent=silent,
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
@click.option(
    "--export-tree",
    is_flag=True,
    default=False,
    show_default=True,
    help="Export tree structure of FTL messages.",
)
def cli_stub(locale_path: Path, output_path: Path, export_tree: bool) -> None:
    click.echo(f"Generating stubs from {locale_path}")
    start_time = perf_counter_ns()

    generate_stubs(locale_path, output_path, export_tree)

    click.echo(f"[Python] Done in {(perf_counter_ns() - start_time) * 1e-9:.3f}s.")
