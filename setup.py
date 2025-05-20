from os import getenv

from setuptools import setup
from setuptools_rust import RustBin, Strip

rust_extensions = []

# Read environment variable `BUILD_RUST_IMPL`, if set to "1" build the Rust implementation
if getenv("BUILD_RUST_IMPL", "0") == "1":
    rust_extensions.append(
        RustBin(
            "fast-ftl-extract",
            path="src/cli/Cargo.toml",
            strip=Strip.All,
            debug=False,
        )
    )

setup(
    package_dir={"ftl_extract": "src/ftl_extract"},
    rust_extensions=rust_extensions,
)
