# Changelog

## 0.12.0 — 2026-09-10

### Breaking changes

Command and configuration renames (already shipped in `0.12.0a1`):

- `ftl untranslated` is gone. Use `ftl check --check untranslated`. The `[tool.ftl-extract.untranslated]` section
  becomes `[tool.ftl-extract.check]`.
- `--fail-on-untranslated` becomes `--fail-on error|warn` (`fail-on = ["error"]` in `pyproject.toml`).
- `--output` / `--output-format` become `--report-path` / `--report-format`. The default report format is now `json`
  (it was `txt`).
- `ftl extract`: the `output-path` config key is now `locales-path`.
- `ftl stub`: `ftl-path` / `output-path` are now `locales-path` / `stub-path`.

Behaviour changes new in `0.12.0`:

- `ftl extract` refuses to write any `.ftl` file and exits `1` when a Python file cannot be read, is not valid UTF-8,
  or does not parse, and when the same key is used with different `_path=` values or different keyword arguments.
  Previously such files were silently skipped, which made their keys look unused and commented them out of every
  locale. Pass `--allow-parse-errors` (or `allow-parse-errors = true`) to skip broken files and continue; key
  conflicts always abort.
- `ftl check` diagnostics now have real severities. `stale` and `untranslated` are warnings; `syntax`, `references`,
  `missing`, `kwargs` and `extraction` are errors. With the default `--fail-on error`, a project whose only problems
  are stale or untranslated keys now exits `0` and prints `FTL check passed with warnings`. Use `--fail-on warn` or a
  severity override to keep failing on them.
- The extraction cache schema is now `v3` (`.ftl-extract-cache/extract-<version>-v3.bin`). Older cache files are
  ignored and rebuilt on the next run.
- A stored `.ftl` message or term that references a message or term that does not exist makes `ftl extract` exit `1`
  with a message naming the entry, the file and the reference. Previously this crashed with a stack trace.
- In the JSON check report, `key` is `null` for file-level extraction errors and `severity` can now be `"warn"`.
- `.gitignore` files inside the locales directory are honoured by `ftl extract` and `ftl check` whether or not the
  project is a git repository (previously only inside one), and `.git/info/exclude` is no longer consulted. Nothing
  above the locales directory or the code directory is read any more, which removes a directory scan per locale.

### Added

- `ftl extract --allow-parse-errors` / `allow-parse-errors = true`: continue past unreadable or unparseable Python
  files instead of aborting. Skipped files are reported as warnings.
- `ftl check --severity <check>=<level>` (repeatable) and `severity = { stale = "error", untranslated = "warn" }`
  under `[tool.ftl-extract.check]` to override the default severity per check. Command-line values win.
- Generalised ignore marker: `# ftl-extract: ignore stale`, `# ftl-extract: ignore untranslated`,
  `# ftl-extract: ignore stale, untranslated`, `# ftl-extract: ignore all`, or a bare `# ftl-extract: ignore` (= all)
  above a message. A message ignored for `stale` keeps everything it references alive, as if Python used it. The older
  `# ftl-extract: ignore-untranslated` spelling still works.
- Extraction diagnostics `parse-error`, `read-error` and `invalid-utf8`, each with the file, line and column. They are
  listed by `ftl extract` before it aborts and reported as `extraction` errors by the `missing`, `stale` and `kwargs`
  checks.
- macOS x86_64 (Intel) wheels are published alongside arm64.

### Fixed

- `.ftl` files, the stub file and the `--export-tree` JSON are written atomically: the content goes to a temporary
  file in the same directory, is flushed, then renamed over the target. An interrupted run leaves the previous file or
  the complete new one, never a truncated one. Write errors are reported for every affected file and exit `1` instead
  of panicking.
- `ftl extract` no longer panics on the same key with two different `_path=` values, on a dangling message or term
  reference, or on reference cycles (`a = { b }`, `b = { a }`).
- `ftl extract` no longer panics on i18n calls without a positional string key, such as `i18n.get(*args)`,
  `i18n.get(**kwargs)`, `i18n.get(key="x")` or `L(name="x")`, nor on a prefixed direct call like
  `self.i18n("key")`, which is now extracted like `i18n("key")`.
- Changing `--exclude-dirs` now invalidates the extraction cache.
- `ftl check` with only warnings no longer reports `FTL check failed`.

### Internal

- Python parser updated to `ruff_python_parser` 0.0.12.
- `fluent-syntax` pinned to a newer `fluent-rs` commit; the `fluent` umbrella crate is no longer a dependency.
- New `common` crate for helpers shared by the extractor, check and stub crates (atomic writes, the `.ftl` directory
  walker, hash aliases, line index). No change to the CLI.

### Migrating a `0.11` `pyproject.toml`

Before (`0.11`):

```toml
[tool.ftl-extract.extract]
code-path = "app/bot"
output-path = "app/bot/locales"
languages = ["en", "uk"]

[tool.ftl-extract.stub]
ftl-path = "app/bot/locales/en"
output-path = "app/bot/stub.pyi"

[tool.ftl-extract.untranslated]
locales-path = "app/bot/locales"
languages = ["uk"]
suggest-from = ["en"]
fail-on-untranslated = true
output = "reports/untranslated"
output-format = "json"
```

After (`0.12`):

```toml
[tool.ftl-extract.extract]
code-path = "app/bot"
locales-path = "app/bot/locales"
languages = ["en", "uk"]

[tool.ftl-extract.stub]
locales-path = "app/bot/locales/en"
stub-path = "app/bot/stub.pyi"

[tool.ftl-extract.check]
locales-path = "app/bot/locales"
code-path = "app/bot"                  # needed by the missing, stale and kwargs checks
languages = ["uk"]
checks = ["untranslated"]              # or ["all"] to run every check
suggest-from = ["en"]
fail-on = ["warn"]                     # untranslated keys are warnings now; "warn" keeps the old failing behaviour
severity = { untranslated = "error" }  # alternatively, make just this check an error and keep fail-on = ["error"]
report-path = "reports/ftl-check"
report-format = "json"
```

Command-line equivalents:

| `0.11`                                                | `0.12`                                                         |
|-------------------------------------------------------|----------------------------------------------------------------|
| `ftl untranslated locales -l uk --suggest-from en`    | `ftl check locales --check untranslated -l uk --suggest-from en` |
| `ftl untranslated ... --fail-on-untranslated`         | `ftl check ... --fail-on warn`                                  |
| `ftl untranslated ... --output r --output-format txt` | `ftl check ... --report-path r --report-format terminal`        |
| `ftl stub locales/en app/stub.pyi`                    | unchanged (positional arguments), only the config keys renamed |
