# Changelog

## 0.12.1 — 2026-09-11

### Breaking changes

- `ftl extract --comment-junks` and the `comment-junks` config key are gone. The flag was dead: a locale file that
  does not parse has aborted `ftl extract` since `0.12.0` (`Failed to parse FTL file ...`) before any commenting
  could happen, and that is the right behavior, because commenting a broken message out and writing a placeholder
  would silently turn a translation into a comment. The CLI flag now fails with clap's `unexpected argument` error.
  A `comment-junks = true` in `pyproject.toml` still loads for the whole `0.12.x` series but does nothing and logs
  `comment-junks has no effect and will be removed in 0.13: syntax errors in .ftl files abort the run`; `0.13`
  drops the key. `ftl config sample` no longer prints it.

### Fixed

- `ftl extract` no longer comments out and replaces a valid translation because of the way its variables are
  written. Three shapes were hit: a variable used only inside a Fluent function call (`{ NUMBER($count) }`, also as
  a selector), which the extractor's own walk skipped; a term reference (`{ -brand(case: "gen") }` or `{ -brand }`),
  who's every variable it counted as one the code had to pass; and a message attribute reference (`{ btn.title }`),
  which it followed to the message value. `items = You have { NUMBER($count) } items` with `i18n.items(count=5)`
  used to become `# items = You have { NUMBER($count) } items` followed by the placeholder `items = items{ $count }`;
  the file is now left byte for byte as it was. For the function-argument and `{ -brand(case: "gen") }` shapes
  `ftl check --check kwargs` passed, so the rewrite went unnoticed; for `{ -brand }` and `{ btn.title }` it failed for
  reasons of its own (see Behavior changes).
- `ftl extract` and `ftl check --check kwargs` no longer disagree about which variables a stored message needs. Both
  now use one collector that follows the `fluent-bundle` resolver: a variable in the value of the message being
  formatted counts, also inside a selector, a nested placeable or a function argument; `{ msg }` pulls in the value
  of `msg` and `{ msg.attr }` only that attribute; nothing inside a term is a caller variable, because a term
  resolves variables against its own call arguments only; and a variable used only in the called message's own
  attributes does not count, because `i18n.key()` renders only the value. What this changes for `check` is listed
  under Behavior changes.
- `ftl extract` no longer merges the last two lines of an entry it comments out, so the commented copy can be
  restored by removing the `# ` prefixes. `old-rules =` with the lines `Rule one.`, `Rule two.` and `Rule three.`
  used to be written as `# old-rules =`, `#     Rule one.`, `#     Rule two.    Rule three.`; it is now
  `# old-rules =`, `#     Rule one.`, `#     Rule two.`, `#     Rule three.`, one `# ` line per serialized line.
- `ftl extract` no longer aborts with `key-message-conflict` when the same key is called with the same keyword
  arguments in a different order (`i18n.get("order", a=1, b=2)` and `i18n.get("order", b=2, a=1)`), in one file or
  across files. A real conflict (`a, b` versus `a, c`) still aborts, and its message now names the key, both
  keyword-argument sets and both call sites in a stable order (by file, line and column), instead of dumping the
  internal AST:
  `[key-message-conflict] Fluent key order is used with different keyword arguments: a, b and a, c (app/a.py:2:5, app/b.py:3:5)`.
- `ftl extract` no longer comments out and replaces a translation because the code calls the key with `**kwargs`:
  `i18n.get("welcome", **data)` with `welcome = Welcome, { $name }!` used to become `welcome = welcome`, silently.
  Such a call can pass any variable, so the key cannot be verified: `extract` leaves the stored message alone and
  `ftl check --check kwargs` skips the key, both logging
  `key "welcome" is called with **kwargs at app/a.py:3:5; its variables cannot be verified` with `--verbose`. A call
  with `**` also never takes part in the `key-message-conflict` comparison: next to `i18n.get("welcome", name=x)` it
  is the same key, only the calls without `**` are compared with each other, and a new key is written with the
  variables of the first call without `**` (or of the first `**` call when there is no other). The other checks
  treat the key as usual.
- When a locale file does not parse, `ftl extract` now reports the file, line and column and the parser's own
  description, like `ftl check --check syntax` does: `Failed to parse FTL file locales/en/_default.ftl:5:1: Expected
  a token starting with "}"` (plus `(and N more)` for further errors), instead of a debug dump of the error list.

### Behavior changes

- The placeholder `ftl extract` writes for a new key lists its variables sorted by name (`order = order{ $a }{ $b }`)
  instead of in call order, which was not even stable across runs because files are extracted in parallel. Stored
  messages are compared by their set of variables, so nothing already stored is rewritten.
- The extraction cache schema is now `v4` (`.ftl-extract-cache/extract-<version>-v4.bin`): cached keys record
  whether a call passes `**kwargs`. Older cache files are ignored and rebuilt on the next run.
- `ftl extract`: a keyword argument in code that only matches a variable inside a referenced term is now a kwargs
  mismatch, so the stored message is commented out and replaced, exactly as `0.12.0` already did for any other unused
  keyword argument. `-brand = Bot { $suffix }`, `about = About { -brand }` and `i18n.about(suffix=...)` used to be
  left alone, because the term's variable counted as satisfied by the caller.
- `ftl check --check kwargs` no longer requires variables that exist only inside terms. `about = About { -brand }`
  with `-brand = { $case -> ... }` used to report `case` as `missing in code`; an unbound term variable falls back to
  the default variant and never reads the caller's arguments, so the message now passes, and `i18n.about(case=...)`
  in code is reported as `unused in ftl: case` like any other unused keyword argument.
- `ftl check --check kwargs` follows `{ msg }` to the value of `msg` only and `{ msg.attr }` to that attribute only.
  It used to count the value plus every attribute of every referenced message, so `a = { b }` reported the variables
  of `b`'s attributes as missing for `a`.
- `ftl check --check kwargs` no longer requires variables that appear only in the called message's own attributes:
  `i18n.save()` renders only the value of `save`, so `$name` in its `.tooltip` attribute is never read by that call
  and is no longer reported as missing. A keyword argument that matches only such a variable is `unused in ftl`, as
  for any other unused keyword argument. `ftl extract` already worked this way.
- The `# ftl-extract: ignore ...` marker is now read by both commands, and `kwargs` joins `stale` and `untranslated`
  as a name. `ftl extract` keeps a key the code never calls when its marker ignores `stale`, together with the
  messages and terms it references, and keeps a called message whose variables differ from the code when its marker
  ignores `kwargs`; `ftl check --check kwargs` leaves such a message out; `all` covers both. A dynamic key such as
  `i18n.get(f"status-{kind}")` with `# ftl-extract: ignore stale` above `status-ok = OK` used to be commented out by
  `extract` on every run although `check` passed; it is now left alone. Kept keys are not counted as commented or
  updated, produce no warning in `--comment-keys-mode warn`, and are listed with `--verbose` as
  `key "status-ok" is kept: marker ignores stale` (`extract`) or `key "items" is skipped in uk: marker ignores kwargs`
  (`check`). `ignore stale` does not cover a kwargs mismatch, `ignore kwargs` does not cover an uncalled key, a marker
  on a called and matching key changes nothing, and only a comment directly above the message (no blank line in
  between) is a marker. Because a kept key is walked like a called one, a reference to a missing message or term in
  it now aborts `extract` instead of vanishing with the commented-out key.

### Internal

- The variable collector (`common::message_variables`, `common::term_variables`) and the ignore-marker parser
  (`common::IgnoreMarker`) live in the `common` crate and are the only implementations; the extractor and check
  crates no longer carry their own walks and parsers. `src/check/tests/extract_check_agreement.rs` runs
  `ftl check --check kwargs` and `ftl extract` on the same fixtures and fails when they disagree.

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

Behavior changes new in `0.12.0`:

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
- `.gitignore` files inside the locales directory are honored by `ftl extract` and `ftl check` whether the
  project is a git repository (previously only inside one), and `.git/info/exclude` is no longer consulted. Nothing
  above the locales directory or the code directory is read anymore, which removes a directory scan per locale.

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
  file in the same directory, then is renamed over the target. An interrupted run leaves the previous file or
  the complete new one, never a truncated one. Write errors are reported for every affected file and exit `1` instead
  of panicking.
- `ftl extract` no longer panics on the same key with two different `_path=` values, on a dangling message or term
  reference, or on reference cycles (`a = { b }`, `b = { a }`).
- `ftl extract` no longer panics on i18n calls without a positional string key, such as `i18n.get(*args)`,
  `i18n.get(**kwargs)`, `i18n.get(key="x")` or `L(name="x")`, nor on a prefixed direct call like
  `self.i18n("key")`, which is now extracted like `i18n("key")`.
- Changing `--exclude-dirs` now invalidates the extraction cache.
- `ftl check` with only warnings no longer reports `FTL check failed`.
- The source distribution builds again. It shipped the extractor's `Cargo.toml`, which declares the benchmark
  target, without the `benches/` directory, so cargo refused to parse it and `pip install` from the sdist failed.

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
fail-on = ["warn"]                     # untranslated keys are warnings now; "warn" keeps the old failing behavior
severity = { untranslated = "error" }  # alternatively, make just this check an error and keep fail-on = ["error"]
report-path = "reports/ftl-check"
report-format = "json"
```

Command-line equivalents:

| `0.11`                                                | `0.12`                                                           |
|-------------------------------------------------------|------------------------------------------------------------------|
| `ftl untranslated locales -l uk --suggest-from en`    | `ftl check locales --check untranslated -l uk --suggest-from en` |
| `ftl untranslated ... --fail-on-untranslated`         | `ftl check ... --fail-on warn`                                   |
| `ftl untranslated ... --output r --output-format txt` | `ftl check ... --report-path r --report-format terminal`         |
| `ftl stub locales/en app/stub.pyi`                    | unchanged (positional arguments), only the config keys renamed   |
