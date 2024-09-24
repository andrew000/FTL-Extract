code_dir = src
docs_dir = docs
docs_source_dir = $(docs_dir)/source
reports_dir = reports
tests_dir = tests

ifeq ($(OS),Windows_NT)
	mkdir_cmd := $(shell if not exist $(reports_dir) mkdir $(reports_dir))
else
	mkdir_cmd := $(shell mkdir -p $(reports_dir))
endif

.PHONY lint:
lint:
	echo "Running ruff..."
	uv run ruff check --config pyproject.toml --diff $(code_dir) $(tests_dir)

	echo "Running MyPy..."
	uv run mypy --config-file pyproject.toml $(code_dir)

.PHONY format:
format:
	echo "Running ruff check with --fix..."
	uv run ruff check --config pyproject.toml --fix --unsafe-fixes $(code_dir) $(tests_dir)

	echo "Running ruff..."
	uv run ruff format --config pyproject.toml $(code_dir) $(tests_dir)

	echo "Running isort..."
	uv run isort --settings-file pyproject.toml $(code_dir) $(tests_dir)

.PHONY livehtml:
livehtml:
	uv run sphinx-autobuild "$(docs_source_dir)" "$(docs_dir)/_build/html" $(SPHINXOPTS) $(O)

.PHONY test:
test:
	echo "Running tests..."
	uv run pytest -vv --cov=$(code_dir) --cov-report=html --cov-report=term --cov-config=.coveragerc $(tests_dir)

.PHONY test-coverage:
test-coverage:
	echo "Running tests with coverage..."
	$(mkdir_cmd)
	uv run pytest -vv --cov=$(code_dir) --cov-config=.coveragerc --html=$(reports_dir)/tests/index.html tests/
	uv run coverage html -d $(reports_dir)/coverage

.PHONY show-outdated:
show-outdated:
	echo "Waiting for uv to create this feature..."

.PHONY uv-sync:
uv-sync:
	uv sync --extra dev --extra tests --extra docs
