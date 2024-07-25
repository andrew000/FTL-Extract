from __future__ import annotations

from pathlib import Path

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
    "--beauty",
    is_flag=True,
    default=False,
    show_default=True,
    help="Beautify output FTL files.",
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
    beauty: bool = False,
    comment_junks: bool = False,
) -> None:
    click.echo(f"Extracting from {code_path}...")

    extract(
        code_path=code_path,
        output_path=output_path,
        language=language,
        i18n_keys=i18n_keys,
        beauty=beauty,
        comment_junks=comment_junks,
    )
