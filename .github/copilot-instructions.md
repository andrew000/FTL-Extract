# Copilot Instructions for FTL-Extract

## Project overview

FTL-Extract is a mixed Python + Rust project:

- Python package name: `FTL-Extract`
- Rust workspace crates: `src/cli`, `src/extractor`, `src/stub`
- Main user CLI command: `ftl`
- Purpose: extract i18n/Fluent keys from Python code and generate/update `.ftl` files, plus generate stubs

## Core behavior to preserve

- `ftl extract <code_path> <locales_path>` scans Python sources and updates locale files.
- Default output behavior should remain compatible with README docs:
  - Default locale directory: `en`
  - Default target file: `_default.ftl` (unless overridden)
- `_path` kwarg support in i18n calls/routes to select specific `.ftl` destination must remain intact.
- `ftl stub <locale_dir> <code_path>` should generate type stubs based on `.ftl` resources.
- Do not break existing CLI flags/options documented in `README.md`.

## Repository layout

- Python package code: `src/ftl_extract`
- Rust code: `src/*/src/**/*.rs`
- Tests: `tests`
- CI workflows: `.github/workflows`

## Tooling and commands

Prefer `uv` and `just` commands used by this repo:

- Sync environment:
  - `uv sync --all-extras` (CI style)
  - or `just sync`
- Lint:
  - Rust: `just lint rust` (runs `cargo clippy --all-targets --all-features`)
  - Python: `just lint py` (ruff checks)
- Format:
  - Rust: `just format rust` (cargo fix + fmt)
  - Python: `just format py` (ruff fix/format + isort)
- Tests:
  - `just test-cov` (cargo llvm-cov lcov report)
  - `just test` (cargo llvm-cov html)
- Build/package:
  - `just build` (uv wheel/sdist + editable install)
  - Rust release build when needed: `cargo build --release`

When changing code, run the smallest relevant lint/tests first, then broader checks if needed.

## Coding conventions

### Python

- Target: Python `>=3.11, <3.15`
- Ruff is authoritative (`pyproject.toml`)
- Formatting:
  - Max line length: 100
  - Double quotes
- Imports sorted with isort config in `pyproject.toml`
- Keep changes explicit and typed where possible; avoid broad exception handling.

### Rust

- Edition: 2024
- Keep code `clippy`-clean for enabled lints in project workflows.
- Use `cargo fmt` formatting.

## Change guidelines for AI agents

- Make focused, minimal diffs; avoid unrelated refactors.
- Update tests when behavior changes.
- Keep README/CLI docs in sync with any option or behavior changes.
- Preserve backward compatibility for existing extract/stub flows unless explicitly requested otherwise.
- For parser/extractor changes, ensure argument/key extraction semantics remain consistent across Python and Fluent processing paths.

## Validation checklist (before finishing)

1. Lint and/or format for touched language(s).
2. Run relevant tests for changed behavior.
3. Confirm CLI behavior still matches documented usage.
4. Ensure no accidental changes to packaging/CI unless intended.

