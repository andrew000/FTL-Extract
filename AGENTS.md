# Repository Guidelines

## Project Structure & Module Organization

This repository is a Rust workspace packaged as a Python-distributed CLI. Workspace crates live under `src/`: `src/cli` provides the `ftl` binary, `src/extractor` contains Fluent key extraction logic, `src/stub` generates Python stubs, and `src/untranslated` handles untranslated-report workflows. Rust tests and benchmarks are colocated with crates, for example `src/extractor/tests` and `src/extractor/benches`. Repository-level fixtures live in `tests/files`.

## Build, Test, and Development Commands

Use `just` targets when possible:

- `just sync`: install/sync Python development dependencies with `uv`.
- `just format`: run `cargo fix` and `cargo fmt --all`.
- `just format py`: run Ruff fixes/formatting and isort.
- `just lint`: run `cargo clippy --all-targets --all-features`.
- `just lint py`: run Ruff checks.
- `just test`: run `cargo llvm-cov --html`.
- `just test-cov`: write an LCOV report to `lcov.info`.
- `just build`: build the Python wheel and sdist with `uv build`.
- `just bench` or `just bench-cmp baseline`: run extractor Criterion benchmarks.

Use `cargo test --workspace` for a quick correctness pass.

## Coding Style & Naming Conventions

Rust uses edition 2024 with formatting enforced by `cargo fmt`. Use `snake_case` for modules, functions, and variables; `PascalCase` for types and traits; `SCREAMING_SNAKE_CASE` for constants. Keep crate-specific code inside its crate boundary.

Python tooling is configured in `pyproject.toml`: 100-character lines, double quotes, space indentation, Ruff, and isort.

## Testing Guidelines

Prefer focused crate tests near changed code. Use fixtures under `tests/files` or crate-local `tests` directories for extraction behavior. Run `cargo test --workspace` for fast verification, then `just test` when coverage output is needed. For performance-sensitive extractor changes, run `just bench` and compare with `just bench-cmp baseline`.

## Commit & Pull Request Guidelines

Recent history uses short maintenance messages (`Bump`, `Update Justfile`) and conventional-style messages such as `feat(i18n): ...` and `perf(extractor): ...`. Prefer conventional form for behavioral changes: `feat(scope): summary`, `fix(scope): summary`, `perf(scope): summary`, or `chore(scope): summary`.

Pull requests should describe the change, list verification commands run, and link related issues. Include benchmark results for performance work and note generated artifacts such as coverage reports or built distributions.

## Security & Configuration Tips

Do not commit `.venv`, `target`, `build`, `dist`, caches, private keys, or local IDE metadata. Pre-commit hooks check formatting, merge conflicts, TOML/YAML/JSON, and secrets; run them before publishing changes.

## Agent-Specific Instructions

When GPT/Codex needs external library or framework documentation, use Context7 first to resolve the library ID and query current documentation before relying on memory.
