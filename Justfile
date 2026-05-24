set shell := ["bash", "-c"]
set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

lint:
    @echo "Running cargo clippy..."
    cargo clippy --all-targets --all-features

format:
    @echo "Running cargo fix..."
    cargo fix --allow-dirty --all

    @echo "Running cargo fmt..."
    cargo fmt --all

test:
    @echo "Running cargo llvm-cov..."
    cargo llvm-cov --html

test-cov:
    @echo "Running cargo llvm-cov for lcov report..."
    cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

bench name="":
    @echo "Running extractor benchmarks..."
    cargo bench -p extractor --bench extract_bench -- {{ name }}

bench-save name="baseline":
    @echo "Saving benchmark baseline '{{ name }}'..."
    cargo bench -p extractor --bench extract_bench -- --save-baseline {{ name }}

bench-cmp name="baseline":
    @echo "Comparing against benchmark baseline '{{ name }}'..."
    cargo bench -p extractor --bench extract_bench -- --baseline {{ name }}

outdated:
    uv tree --universal --outdated --no-cache --depth=1
    cargo outdated -w

sync:
    uv sync --no-install-project --group dev

[windows]
clean:
    @echo "Removing build artifacts..."
    Remove-Item -LiteralPath build, dist -Recurse -Force -ErrorAction Ignore; exit 0
    Get-ChildItem -Path . -Directory -Filter "*.egg-info" -Force | ForEach-Object { Remove-Item -LiteralPath $_.FullName -Recurse -Force }

[unix]
clean:
    @echo "Removing build artifacts..."
    rm -rf build dist ./*.egg-info

build: clean
    uv build --wheel --sdist
