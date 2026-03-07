set shell := ["bash", "-c"]
set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

py_code_dir := "src/ftl_extract"
docs_dir := "docs"
docs_source_dir := docs_dir / "source"
reports_dir := "reports"
tests_dir := "tests"

lint target="py":
    @{{ if target == "py" { "just _lint-py" } else if target == "rust" { "just _lint-rust" } else { "echo \"Unknown target: " + target + ". Please specify 'py' or 'rust'.\"" } }}

_lint-py:
    @echo "Running ruff..."
    uv run ruff check --config pyproject.toml --diff --unsafe-fixes {{ py_code_dir }} {{ tests_dir }}

_lint-rust:
    @echo "Running cargo clippy..."
    cargo clippy --all-targets --all-features

format target="py":
    @{{ if target == "py" { "just _format-py" } else if target == "rust" { "just _format-rust" } else { "echo \"Unknown target: " + target + ". Please specify 'py' or 'rust'.\"" } }}

_format-py:
    @echo "Running ruff check with --fix"
    uv run ruff check --config pyproject.toml --fix --unsafe-fixes {{ py_code_dir }} {{ tests_dir }}

    @echo "Running ruff..."
    uv run ruff format --config pyproject.toml {{ py_code_dir }} {{ tests_dir }}

    @echo "Running isort..."
    uv run isort --settings-path pyproject.toml {{ py_code_dir }} {{ tests_dir }}

_format-rust:
    @echo "Running cargo fix..."
    cargo fix --allow-dirty --all

    @echo "Running cargo fmt..."
    cargo fmt --all

test-py:
    @echo "Running pytest..."
    uv run pytest \
        -vv \
        --cov={{ py_code_dir }} \
        --cov-report=html \
        --cov-report=term \
        {{ tests_dir }}

test-py-cov:
    @echo "Running pytest for coverage report..."
    uv run pytest \
        -vv \
        --cov={{ py_code_dir }} \
        --cov-branch \
        --cov-report=xml \
        --cov-report=term \
        {{ tests_dir }}

test-rust:
    @echo "Running cargo llvm-cov..."
    cargo llvm-cov --html

test-rust-cov:
    @echo "Running cargo llvm-cov for lcov report..."
    cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

outdated:
    uv tree --universal --outdated --no-cache --depth=1

sync:
    uv sync --reinstall-package ftl_extract --all-extras

[windows]
build:
    uv build --wheel --sdist
    uv pip install -e .

[unix]
build:
    uv build --wheel --sdist
    uv pip install -e .
