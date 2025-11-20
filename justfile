set shell := ["bash", "-c"]
set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

py_code_dir := "src/ftl_extract"
docs_dir := "docs"
docs_source_dir := docs_dir / "source"
reports_dir := "reports"
tests_dir := "tests"

lint:
    @echo "Running ruff..."
    uv run ruff check --config pyproject.toml --diff --unsafe-fixes {{ py_code_dir }} {{ tests_dir }}

    @echo "Running mypy..."
    uv run mypy --config-file pyproject.toml

format:
    @echo "Running ruff check with --fix"
    uv run ruff check --config pyproject.toml --fix --unsafe-fixes {{ py_code_dir }} {{ tests_dir }}

    @echo "Running ruff..."
    uv run ruff format --config pyproject.toml {{ py_code_dir }} {{ tests_dir }}

    @echo "Running isort..."
    uv run isort --settings-path pyproject.toml {{ py_code_dir }} {{ tests_dir }}

py-test:
    @echo "Running pytest..."
    uv run pytest \
        -vv \
        --cov={{ py_code_dir }} \
        --cov-report=html \
        --cov-report=term \
        --cov-config=.coveragerc \
        {{ tests_dir }}

rust-test:
    @echo "Running cargo test..."
    $RUSTFLAGS="-C instrument-coverage" && \
      cargo test --tests

outdated:
    uv tree --universal --outdated --no-cache

sync:
    uv sync --reinstall-package ftl_extract --all-extras

build:
    uv build --wheel --sdist
