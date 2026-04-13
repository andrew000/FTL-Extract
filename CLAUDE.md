# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

FTL-Extract extracts Fluent (FTL) translation keys from Python source code and manages `.ftl` locale files. The CLI binary (`ftl`) is written in Rust and ships inside a Python wheel via `setuptools-rust`.

Three subcommands:
- `ftl extract <code_path> <output_path>` — scan Python sources, create/update FTL files
- `ftl stub <ftl_path> <output_path>` — generate `.pyi` type stubs from FTL files
- `ftl untranslated <locales_path>` — report missing/placeholder translations across locales

## Commands

```bash
just lint rust                        # cargo clippy --all-targets --all-features
just format rust                      # cargo fix --allow-dirty --all && cargo fmt --all
just test                             # cargo llvm-cov --html
just test-cov                         # cargo llvm-cov (lcov output, used in CI)
cargo test <test_name>                # run a single test
cargo test -p extractor <test_name>   # single test scoped to a crate
just build                            # uv build --wheel --sdist
```

## Architecture

Rust workspace with four crates (three libraries, one binary):

**`cli`** (`src/cli/`) — Binary entry point. `clap` for arg parsing, `mimalloc` allocator. Orchestrates the three library crates.

**`extractor`** (`src/extractor/`) — Core extraction engine. Walks Python files (`ignore` crate, respects `.gitignore`), parses AST with Ruff's Python parser (`ruff_python_ast`/`ruff_python_parser`), extracts i18n keys, diffs against existing FTL files, serializes output. Uses `rayon` for parallel extraction and `memchr` for quick pre-filtering. Key types: `FluentKey`, `FluentEntry`, `ExtractConfig`, `FastHashMap`/`FastHashSet` (FxHasher aliases).

**`stub`** (`src/stub/`) — Generates `.pyi` type stubs from FTL files. Builds a tree from flat FTL keys (split on `-`), produces nested Python classes with `@overload` for dual-purpose keys.

**`untranslated`** (`src/untranslated/`) — Detects placeholder translations (key == value), supports `# ftl-extract: ignore-untranslated` comment markers, suggests translations from source locales. Outputs terminal, TXT, or JSON reports.

The library crates have **no inter-dependencies** — only `cli` depends on all three.

### Test fixtures

`tests/files/py/` contains Python source files used as fixtures by Rust tests (not a Python test suite).

### Git-pinned dependencies

- `fluent`/`fluent-syntax` — pinned to a specific `fluent-rs` git revision
- `ruff_python_ast`/`ruff_python_parser` — pinned to ruff git tag `0.15.9`

Upgrading either requires manual coordination.

## Conventions

- Rust edition 2024, `rust-version = "1.93"`
- `cargo clippy` and `cargo fmt` are authoritative for Rust style
- Pre-commit hooks enforce trailing whitespace, CRLF line endings for Rust/YAML/TOML, no private keys
- CI runs on `dev` branch; coverage collected on Linux x86_64 manylinux only
- Wheels built with `cibuildwheel`, tags rewritten to `py3-none`

## Behaviors to preserve

- Default locale directory: `en`, default target file: `_default.ftl`
- `_path` kwarg in i18n calls routes keys to specific `.ftl` files
- Key conflicts (same key, different paths) intentionally `panic!`
- Do not break existing CLI flags documented in README.md
