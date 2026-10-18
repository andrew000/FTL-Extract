# Configuration

Every option can be given on the command line or stored in `pyproject.toml`, one table per command:

```toml
[tool.ftl-extract.extract]
[tool.ftl-extract.stub]
[tool.ftl-extract.check]
```

## How options are resolved

1. A value passed on the command line wins.
2. Otherwise the value from `pyproject.toml` is used.
3. Otherwise the built-in default applies.

List options are replaced, not merged: `-k L` on the command line replaces the whole `i18n-keys` list from
`pyproject.toml`. Use the `*-append` options to add to the default list instead.

`ftl` looks for `pyproject.toml` in the current directory and then in each parent directory. Pass `--config` to use
a specific file:

```shell
ftl --config ./pyproject.toml extract
```

Relative paths in `pyproject.toml` are resolved from the directory that contains the file, not from the current
directory, so the commands work from anywhere inside the project.

To print a commented sample:

```shell
ftl config sample
ftl config sample --command check
```

## `[tool.ftl-extract.extract]`

| Key                        | Type            | Default                | CLI                              |
|----------------------------|-----------------|------------------------|----------------------------------|
| `code-path`                | path            | required               | 1st argument                     |
| `locales-path`             | path            | required               | 2nd argument                     |
| `languages`                | list of strings | `["en"]`               | `-l`, `--language`               |
| `i18n-keys`                | list of strings | see below              | `-k`, `--i18n-keys`              |
| `i18n-keys-append`         | list of strings | `[]`                   | `-K`, `--i18n-keys-append`       |
| `i18n-keys-prefix`         | list of strings | `[]`                   | `-p`, `--i18n-keys-prefix`       |
| `exclude-dirs`             | list of globs   | see below              | `-e`, `--exclude-dirs`           |
| `exclude-dirs-append`      | list of globs   | `[]`                   | `-E`, `--exclude-dirs-append`    |
| `ignore-attributes`        | list of strings | see below              | `-i`, `--ignore-attributes`      |
| `ignore-attributes-append` | list of strings | `[]`                   | `-I`, `--append-ignore-attributes` |
| `ignore-kwargs`            | list of strings | `[]`                   | `--ignore-kwargs`                |
| `default-ftl-file`         | path            | `"_default.ftl"`       | `--default-ftl-file`             |
| `comment-keys-mode`        | `"comment"`, `"warn"` | `"comment"`      | `--comment-keys-mode`            |
| `line-endings`             | `"default"`, `"lf"`, `"cr"`, `"crlf"` | `"default"` | `--line-endings`         |
| `dry-run`                  | bool            | `false`                | `--dry-run`                      |
| `cache`                    | bool            | `false`                | `--cache`                        |
| `cache-path`               | path            | `.ftl-extract-cache/`  | `--cache-path`                   |
| `clear-cache`              | bool            | `false`                | `--clear-cache`                  |
| `allow-parse-errors`       | bool            | `false`                | `--allow-parse-errors`           |
| `leak-memory`              | bool            | `false`                | `--leak-memory`                  |

Default lists:

`i18n-keys`
:   `i18n`, `L`, `LazyProxy`, `LazyFilter`

`exclude-dirs`
:   `**/.venv/**`, `**/venv/**`, `**/.git/**`, `**/__pycache__/**`, `**/.pytest_cache/**`. Files ignored by
    `.gitignore` are skipped too.

`ignore-attributes`
:   `set_locale`, `use_locale`, `use_context`, `set_context`

What each option does is described in [ftl extract](commands/extract.md#options).

!!! info "`ftl check` reads this table too"

    The `missing`, `stale` and `kwargs` checks scan Python code the same way `ftl extract` does. They take
    `i18n-keys`, `i18n-keys-append`, `i18n-keys-prefix`, `exclude-dirs`, `exclude-dirs-append`, `ignore-attributes`,
    `ignore-attributes-append`, `ignore-kwargs`, `default-ftl-file` and the cache settings from
    `[tool.ftl-extract.extract]`; `ftl check` has no command-line flags for them. When `[tool.ftl-extract.check]`
    has no `code-path`, the one from the `extract` table is used.

## `[tool.ftl-extract.stub]`

| Key            | Type | Default  | CLI             |
|----------------|------|----------|-----------------|
| `locales-path` | path | required | 1st argument    |
| `stub-path`    | path | required | 2nd argument    |
| `export-tree`  | bool | `false`  | `--export-tree` |

`locales-path` is the directory of a single locale, for example `app/locales/en`. `stub-path` is the `.pyi` file to
write.

## `[tool.ftl-extract.check]`

| Key             | Type                         | Default        | CLI                  |
|-----------------|------------------------------|----------------|----------------------|
| `locales-path`  | path                         | required       | 1st argument         |
| `code-path`     | path                         | from `extract` | `--code-path`        |
| `languages`     | list of strings              | all locales    | `-l`, `--language`   |
| `checks`        | list of check names          | `["all"]`      | `--check`            |
| `suggest-from`  | list of strings              | `[]`           | `--suggest-from`     |
| `fail-on`       | list of `"error"`, `"warn"`  | `["error"]`    | `--fail-on`          |
| `severity`      | table of check = severity    | see below      | `--severity`         |
| `report-path`   | path                         | none           | `--report-path`      |
| `report-format` | `"json"`, `"terminal"`       | `"json"`       | `--report-format`    |

Check names: `all`, `syntax`, `references`, `missing`, `stale`, `kwargs`, `untranslated`. The default severities are
listed under [Severities](commands/check.md#severities).

```toml
[tool.ftl-extract.check]
severity = { stale = "error", untranslated = "warn" }
```

## Full example

```toml title="pyproject.toml"
[tool.ftl-extract.extract]
code-path = "app/bot"
locales-path = "app/bot/locales"
languages = ["en", "uk"]
i18n-keys-append = ["LF"]
ignore-attributes-append = ["core"]
exclude-dirs-append = ["./tests/*"]
ignore-kwargs = ["when"]
comment-keys-mode = "comment"
line-endings = "lf"
cache = true

[tool.ftl-extract.stub]
locales-path = "app/bot/locales/en"
stub-path = "app/bot/stub.pyi"

[tool.ftl-extract.check]
locales-path = "app/bot/locales"
languages = ["uk"]
suggest-from = ["en"]
severity = { stale = "error" }
report-path = "reports/ftl-check"
```
