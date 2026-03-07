from __future__ import annotations

import argparse
import logging
import subprocess
import sys

logging.basicConfig(level=logging.INFO, format="%(message)s")
logger = logging.getLogger(__name__)

_RUST_COMMANDS = {"extract", "stub"}


def main() -> None:
    """Entry point for the FTL CLI."""
    # Proxy extract and stub commands directly to the Rust binary before argparse
    if len(sys.argv) > 1 and sys.argv[1] in _RUST_COMMANDS:
        try:
            result = subprocess.run(["fast-ftl", *sys.argv[1:]], check=False)  # noqa: S603, S607
            sys.exit(result.returncode)
        except FileNotFoundError:
            logger.error("Error: 'fast-ftl' executable not found.")  # noqa: TRY400
            logger.error("Please ensure the Rust implementation is built and in your PATH.")  # noqa: TRY400
            sys.exit(1)

    parser = argparse.ArgumentParser(
        prog="ftl", description="Fast Fluent CLI for i18n key extraction and stub generation"
    )
    parser.add_argument("-V", "--version", action="version", version="%(prog)s " + get_version())

    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # Subcommand entries for help display only — actual handling is proxied to Rust above
    subparsers.add_parser(
        "stub",
        help="Generate Python stubs from FTL files (handled by Rust binary)",
    )
    subparsers.add_parser(
        "extract",
        help="Extract i18n keys to FTL files (handled by Rust binary)",
    )

    args = parser.parse_args()

    if args.command in _RUST_COMMANDS:
        # Shouldn't be reached due to early interception above
        logger.error("Error: '%s' command should have been handled by Rust binary", args.command)
        sys.exit(1)
    else:
        parser.print_help()


def get_version() -> str:
    """Get version from package."""
    from ftl_extract.__version__ import __version__  # noqa: PLC0415

    return __version__
