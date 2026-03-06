from __future__ import annotations

import argparse
import logging
import subprocess
import sys
from pathlib import Path
from time import perf_counter_ns

from ftl_extract.stub.generator import generate_stubs

logging.basicConfig(level=logging.INFO, format="%(message)s")
logger = logging.getLogger(__name__)


def main() -> None:
    """Entry point for the FTL CLI."""
    # Handle extract command directly before argparse to proxy to Rust
    if len(sys.argv) > 1 and sys.argv[1] == "extract":
        try:
            rust_cmd = ["fast-ftl", *sys.argv[1:]]
            # Use subprocess.run instead of execvp for better Windows compatibility
            result = subprocess.run(rust_cmd, check=False)  # noqa: S603
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

    # Stub command
    stub_parser = subparsers.add_parser("stub", help="Generate Python stubs from FTL files")
    stub_parser.add_argument(
        "locale_path", type=Path, help="Path to directory containing FTL files"
    )
    stub_parser.add_argument(
        "output_path", type=Path, help="Path where Python stubs should be generated"
    )
    stub_parser.add_argument(
        "--export-tree", action="store_true", help="Export tree structure of FTL messages"
    )

    # Extract command (just for help display - actual handling is above)
    extract_parser = subparsers.add_parser("extract", help="Extract i18n keys to FTL files")
    extract_parser.add_argument(
        "--help-rust",
        action="store_true",
        help="This command is handled by the Rust binary. "
        "Use 'fast-ftl extract --help' for full options.",
    )

    args = parser.parse_args()

    if args.command == "stub":
        handle_stub_command(args)
    elif args.command == "extract":
        # This shouldn't be reached due to early interception above
        logger.error("Error: Extract command should have been handled by Rust binary")
        sys.exit(1)
    else:
        parser.print_help()


def get_version() -> str:
    """Get version from package."""
    from ftl_extract.__version__ import __version__  # noqa: PLC0415

    return __version__


def handle_stub_command(args: argparse.Namespace) -> None:
    """Handle the stub command."""
    if not args.locale_path.exists():
        logger.error("Error: Locale path '%s' does not exist.", args.locale_path)
        sys.exit(1)

    logger.info("Generating stubs from %s", args.locale_path)
    start_time = perf_counter_ns()

    generate_stubs(args.locale_path, args.output_path, args.export_tree)

    logger.info("[Python] Done in %.3fs.", (perf_counter_ns() - start_time) * 1e-9)
