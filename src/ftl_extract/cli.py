from __future__ import annotations

from pathlib import Path
from time import perf_counter_ns

import click

from ftl_extract.const import COMMENT_KEYS_MODE, DEFAULT_FTL_FILE, IGNORE_KWARGS
from ftl_extract.ftl_extractor import extract


@click.command()
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
    "--i18n_keys",
    "-k",
    default=("i18n", "L", "LazyProxy", "LazyFilter"),
    multiple=True,
    show_default=True,
    help="Names of function that is used to get translation.",
)
@click.option(
    "--ignore-attributes",
    default=("set_locale", "use_locale", "use_context", "set_context"),
    multiple=True,
    show_default=True,
    help="Ignore attributes, like `i18n.set_locale`.",
)
@click.option(
    "--expand-ignore-attributes",
    "-a",
    multiple=True,
    help="Expand default|targeted ignore attributes.",
)
@click.option(
    "--ignore-kwargs",
    default=IGNORE_KWARGS,
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
    default="comment",
    show_default=True,
    help="Comment keys mode.",
    type=click.Choice(COMMENT_KEYS_MODE),
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
@click.version_option()
def cli_extract(
    code_path: Path,
    output_path: Path,
    language: tuple[str, ...],
    i18n_keys: tuple[str, ...],
    ignore_attributes: tuple[str, ...],
    expand_ignore_attributes: tuple[str, ...] | None = None,
    ignore_kwargs: tuple[str, ...] = (),
    comment_junks: bool = False,
    default_ftl_file: Path = DEFAULT_FTL_FILE,
    comment_keys_mode: str = "comment",
    dry_run: bool = False,
    verbose: bool = False,
) -> None:
    click.echo(f"Extracting from {code_path}")
    start_time = perf_counter_ns()

    statistics = extract(
        code_path=code_path,
        output_path=output_path,
        language=language,
        i18n_keys=i18n_keys,
        ignore_attributes=ignore_attributes,
        expand_ignore_attributes=expand_ignore_attributes,
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

    click.echo(f"Done in {(perf_counter_ns() - start_time) * 1e-9:.3f}s.")
