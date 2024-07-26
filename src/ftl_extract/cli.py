from __future__ import annotations

from pathlib import Path
from time import perf_counter_ns

import click

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
    "--comment-junks",
    is_flag=True,
    default=False,
    show_default=True,
    help="Comments Junk elements.",
)
@click.version_option()
def cli_extract(
    code_path: Path,
    output_path: Path,
    language: tuple[str, ...],
    i18n_keys: tuple[str, ...],
    ignore_attributes: tuple[str, ...],
    expand_ignore_attributes: tuple[str, ...] | None = None,
    comment_junks: bool = False,
) -> None:
    click.echo(f"Extracting from {code_path}...")
    start_time = perf_counter_ns()

    extract(
        code_path=code_path,
        output_path=output_path,
        language=language,
        i18n_keys=i18n_keys,
        ignore_attributes=ignore_attributes,
        expand_ignore_attributes=expand_ignore_attributes,
        comment_junks=comment_junks,
    )

    click.echo(f"Done in {(perf_counter_ns() - start_time) * 1e-9:.3f}s.")
