# `ftl check`

```
ftl check [OPTIONS] [LOCALES_PATH]
```

Validates the locales under `LOCALES_PATH` and, with `--code-path`, compares them with the Python code. It never
changes any file, so it is safe to run in CI and pre-commit hooks.

```shell
ftl check app/locales --code-path app -l uk --suggest-from en
```

```
FTL check failed: 2 problems

Locale: uk

error[missing]: key `button-help` is used in code but missing in locale `uk`
  file: uk/buttons.ftl
  code: app/handlers.py:12:10

error[kwargs]: key `welcome` in locale `uk` has variable mismatch
  file: uk/_default.ftl:2
  code: app/handlers.py:5:12
  missing in code: user
  unused in ftl: name

Summary:
- Errors: 2
- Warnings: 0
- Checks:
  - kwargs: failed (1 errors, 0 warnings)
  - missing: failed (1 errors, 0 warnings)
  - references: passed
  - stale: passed
  - syntax: passed
  - untranslated: passed
```

The options can come from [`[tool.ftl-extract.check]`](../configuration.md#toolftl-extractcheck). The way Python
code is scanned (`i18n-keys`, `exclude-dirs` and so on) is read from `[tool.ftl-extract.extract]`, so `ftl check`
finds exactly the keys `ftl extract` finds.

## Checks

| Check          | Needs `--code-path` | Reports                                                                 |
|----------------|:-------------------:|-------------------------------------------------------------------------|
| `syntax`       |                     | `.ftl` files that do not parse, with line and column.                   |
| `references`   |                     | `{ message }` or `{ -term }` references to entries that do not exist.   |
| `untranslated` |                     | Messages whose value is exactly the key, like `button-help = button-help`. A placeholder with variables, such as `welcome = welcome{ $name }`, is not detected. |
| `missing`      | yes                 | Keys used in code that the locale does not provide.                     |
| `stale`        | yes                 | Messages in the locale that code no longer uses.                        |
| `kwargs`       | yes                 | Messages whose variables differ from the keyword arguments in code.     |
| `extraction`   | yes                 | Python files that cannot be read or parsed. Reported by the three checks above, since their results are incomplete while such files exist. |

`--check all`, or no `--check` at all, runs every check, so it needs a code path too; without one the command exits
with `2`. To check only the locale files, list the checks: `--check syntax --check references --check untranslated`.
The `syntax` check runs first: if it finds errors, the other checks are skipped, because broken files cannot be
analyzed.

The `stale` check treats a message referenced by another message as used. A key called with `**kwargs` is skipped by
the `kwargs` check, because its variables cannot be known; see [Variables and kwargs](../concepts/kwargs.md).

Keys are compared together with the file they belong to. A key that lives in a different file than the code expects is
reported twice, as `missing` and as `stale`; see [File layout](../concepts/file-layout.md#keys-are-matched-per-file).

## Severities

Every diagnostic is an `error` or a `warn`. Errors mean the application is broken or will break at runtime; warnings
mean the catalogue is untidy.

| Check          | Default | Why                                                               |
|----------------|---------|-------------------------------------------------------------------|
| `syntax`       | `error` | The `.ftl` file cannot be loaded.                                 |
| `references`   | `error` | A message references something that does not exist.               |
| `missing`      | `error` | Code uses a key the locale does not provide.                      |
| `kwargs`       | `error` | Code passes different variables than the message expects.        |
| `extraction`   | `error` | A Python file could not be analyzed, so other results are incomplete. |
| `stale`        | `warn`  | The locale contains a key code no longer uses.                    |
| `untranslated` | `warn`  | A message still equals its key.                                   |

Change a severity with `--severity`, which can be repeated:

```shell
ftl check app/locales --code-path app --severity stale=error
```

or in `pyproject.toml`:

```toml
[tool.ftl-extract.check]
severity = { stale = "error", untranslated = "warn" }
```

Command-line overrides take precedence. Syntax errors stop the remaining checks even when their severity is `warn`.

## Exit code

`--fail-on` sets the lowest severity that makes the command exit with `1`:

| `fail-on`                 | Exits `1` on                                   |
|---------------------------|------------------------------------------------|
| `error` (default)         | errors. Warnings print `FTL check passed with warnings`. |
| `warn`                    | errors and warnings.                           |
| `fail-on = []` in pyproject | never: report only.                          |

## Reports

The result is always printed to the terminal. To also save it, pass `--report-path`:

```shell
ftl check app/locales --code-path app --report-path reports/ftl-check
```

`--report-format` selects `json` (the default) or `terminal`. Without an extension, `.json` or `.txt` is added to the
path. The JSON report has this shape:

```json
{
  "ok": false,
  "diagnostics": [
    {
      "kind": "kwargs",
      "severity": "error",
      "locale": "uk",
      "key": "welcome",
      "message": "key `welcome` in locale `uk` has variable mismatch",
      "ftl_location": { "path": "uk/_default.ftl", "line": 2, "column": null },
      "code_location": { "path": "app/handlers.py", "line": 5, "column": 12 },
      "missing_kwargs": ["user"],
      "unused_kwargs": ["name"],
      "suggestions": []
    }
  ],
  "summary": {
    "errors": 1,
    "warnings": 0,
    "checks": [
      { "kind": "kwargs", "passed": false, "errors": 1, "warnings": 0 }
    ]
  }
}
```

`ftl_location` paths are relative to `LOCALES_PATH`. Fields that do not apply to a diagnostic are `null` or empty
lists. `suggestions` holds translations found in the `--suggest-from` locales.

## Options

`--code-path <PATH>`
:   Python code to compare with. Required by `missing`, `stale` and `kwargs`. Falls back to `code-path` from
    `[tool.ftl-extract.extract]`.

`--check <CHECK>`
:   A check to run: `all`, `syntax`, `references`, `untranslated`, `missing`, `stale`, `kwargs`. Repeatable.

`-l`, `--language <CODE>`
:   Check only this locale. Repeatable. Default: every locale directory. An unknown locale exits with `2`.

`--suggest-from <CODE>`
:   Locales to take suggested translations from for missing and untranslated messages. Only real translations are
    suggested, not placeholders. Repeatable:

    ```
    warn[untranslated]: key `button-help` is untranslated in locale `uk`
      file: uk/buttons.ftl:1
      suggestion[en]: button-help = Help
    ```

`--fail-on <SEVERITY>`
:   `error` or `warn`. See [Exit code](#exit-code).

`--severity <CHECK=SEVERITY>`
:   Override the severity of a check. Repeatable.

`--report-path <PATH>`
:   Also write the report to this file.

`--report-format <FORMAT>`
:   `json` (default) or `terminal`.

## Example configurations

=== "Everything"

    ```toml
    [tool.ftl-extract.check]
    locales-path = "app/bot/locales"
    code-path = "app/bot"
    languages = ["uk", "pl"]
    suggest-from = ["en"]
    report-path = "reports/ftl-check"
    ```

=== "Locale files only"

    No `code-path` needed:

    ```toml
    [tool.ftl-extract.check]
    locales-path = "app/bot/locales"
    checks = ["syntax", "references", "untranslated"]
    report-format = "terminal"
    ```

=== "Strict CI"

    ```toml
    [tool.ftl-extract.check]
    locales-path = "app/bot/locales"
    code-path = "app/bot"
    fail-on = ["warn"]
    ```
