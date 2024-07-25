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

lint:
	@echo "Running ruff..."
	@poetry run ruff check --config pyproject.toml --diff $(code_dir) $(tests_dir)

	@echo "Running MyPy..."
	@poetry run mypy --config-file pyproject.toml $(code_dir)

format:
	@echo "Running ruff check with --fix..."
	@poetry run ruff check --config pyproject.toml --fix --unsafe-fixes $(code_dir) $(tests_dir)

	@echo "Running ruff..."
	@poetry run ruff format --config pyproject.toml $(code_dir) $(tests_dir)

	@echo "Running isort..."
	@poetry run isort --settings-file pyproject.toml $(code_dir) $(tests_dir)

livehtml:
	@sphinx-autobuild "$(docs_source_dir)" "$(docs_dir)/_build/html" $(SPHINXOPTS) $(O)

test:
	@echo "Running tests..."
	@poetry run pytest -vv --cov=$(code_dir) --cov-report=html --cov-report=term --cov-config=.coveragerc $(tests_dir)

test-coverage:
	@echo "Running tests with coverage..."
	@$(mkdir_cmd)
	@poetry run pytest -vv --cov=$(code_dir) --cov-config=.coveragerc --html=$(reports_dir)/tests/index.html tests/
	@poetry run coverage html -d $(reports_dir)/coverage
