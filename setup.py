from setuptools import setup
from setuptools_rust import RustBin, Strip

setup(
    package_dir={"ftl_extract": "src/ftl_extract"},
    rust_extensions=[
        RustBin(
            "fast-ftl-extract",
            path="src/cli/Cargo.toml",
            strip=Strip.All,
        )
    ],
)
