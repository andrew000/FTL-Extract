# FTL-Extract threat model

## What the project does

FTL-Extract is a command-line tool, `ftl`, written in Rust and shipped as a native binary inside a
Python wheel. It keeps the [Fluent](https://projectfluent.org/) translation files of a Python project
in sync with the code:

- `ftl extract` parses every `.py` file under a code path, finds translation calls such as
  `i18n.get("key", name=...)` or `i18n.menu.title()`, and writes or updates `.ftl` files for each
  locale under a locales path. Unused keys are commented out, existing translations are kept.
- `ftl stub` reads the `.ftl` files of one locale and generates a Python `.pyi` stub.
- `ftl check` parses `.ftl` files (and optionally the code) and reports syntax errors, missing keys,
  wrong variables, broken references and untranslated messages.

It runs on developer machines, in pre-commit hooks and in CI, usually on a checkout that may contain
changes from people other than the person running the tool.

## Where untrusted input enters

The person who runs `ftl` controls its command-line arguments and `pyproject.toml`
(`[tool.ftl-extract.*]`). Those are trusted. The files the tool reads are not:

1. **Python source files** under the code path. They may come from a contributor's pull request,
   vendored code or generated code. They are parsed with `ruff_python_parser` and walked in
   `src/extractor/src/ftl/code_extractor.rs` and `matcher.rs`. Literal values from the code end up in
   the output: key names, keyword-argument names and the `_path="..."` argument, which picks the
   `.ftl` file a key is written to.
2. **`.ftl` files** under the locales path. Translators and translation platforms (Weblate, Crowdin,
   pull requests from the community) edit them. They are parsed with `fluent-syntax` and processed by
   `src/extractor/src/ftl/ftl_importer.rs`, `src/check` and `src/stub`. Their content is written back
   by `ftl extract` and embedded in generated Python by `ftl stub`.
3. **The extraction cache**, `.ftl-extract-cache/extract-<version>-v<schema>.bin` by default, decoded
   with `bincode-next` in `src/extractor/src/ftl/cache.rs`. It is meant to be local, but CI caches
   and checked-in or shared directories can hand a run a file someone else wrote. Cached entries hold
   file paths that the extractor later uses.

## Security properties the tool should keep

- Every file `ftl extract` writes stays inside the configured locales path (or at the configured
  cache path), whatever the Python and `.ftl` input says. No input should make it create, overwrite
  or delete files elsewhere, including through `_path`, `..`, absolute paths, symlinks or a crafted
  cache.
- A stub generated from `.ftl` files is inert: message names, variables and text cannot inject
  Python code or break out of the string literals and identifiers they are placed in.
- `ftl check` does not report success for input that the extractor or a Fluent runtime would treat
  differently in a way that hides a problem.
- Reading a crafted input file does not crash the process with memory-unsafe behavior, and does not
  hang or exhaust memory out of proportion to the input size.
- Text printed to the terminal (paths, keys, diagnostics) does not let an input file inject terminal
  control sequences.

## Priority components

1. Path handling for output files: `_path` in `matcher.rs`, `write_results` and `write` in
   `ftl_extractor.rs`, `src/common/src/atomic_write.rs`, cache paths in `cache.rs`.
2. Stub generation: `src/stub/src/generator.rs` and `src/stub/src/tree.rs`.
3. Cache decoding: `src/extractor/src/ftl/cache.rs`.
4. Python extraction: `code_extractor.rs`, `matcher.rs`, `kwargs_extractor.rs`.
5. `.ftl` import, serialization and commenting: `ftl_importer.rs`, `process/serializer.rs`,
   `process/commentator.rs`.
6. Checks and their terminal and JSON reports: `src/check`.

## Severity guidance

- **Critical**: code execution on the machine running `ftl` caused by a Python, `.ftl` or cache file.
- **High**: creating, overwriting or deleting a file outside the locales path or cache path from file
  content; injecting executable Python into a generated stub.
- **Medium**: a crash, hang or memory blow-up from a small crafted file that breaks CI for everyone
  using the repository; silently losing or corrupting existing translations; `ftl check` passing on
  input it should reject.
- **Low**: terminal control sequence injection in printed output; misleading diagnostics.

## Out of scope

- Anything that needs control over command-line arguments or `pyproject.toml`: whoever sets them can
  already point the tool at any path.
- Resource use that grows linearly with input size, such as a very large code tree.
- Wrong or missing extraction of keys with no security impact; report those as normal bugs.
- Vulnerabilities in dependencies that are not reachable through `ftl`.
- The benchmarks under `src/extractor/benches` and the Python packaging metadata.

## Proofs of concept and patches

The image has the `ftl` binary on `PATH` (`/src/target/debug/ftl`) and the test suite builds offline
with `cargo test --workspace`. A good proof of concept is a small directory with the `.py` and
`.ftl` files and the exact `ftl` command, or a Rust test in the affected crate.

Patches should be minimal, add a regression test next to the changed code, keep the existing style
(`cargo fmt --all`, `cargo clippy --all-targets --all-features`) and add no new dependencies.
