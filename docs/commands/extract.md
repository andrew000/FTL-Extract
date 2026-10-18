# `ftl extract`

```
ftl extract [OPTIONS] [CODE_PATH] [LOCALES_PATH]
```

Scans the Python files under `CODE_PATH` for translation keys and updates the `.ftl` files under `LOCALES_PATH`, one
directory per locale:

```
LOCALES_PATH/
├── en/
│   └── _default.ftl
└── uk/
    └── _default.ftl
```

Both paths can come from [`[tool.ftl-extract.extract]`](../configuration.md#toolftl-extractextract) instead.

## What changes in the `.ftl` files

For every locale, `ftl extract` compares the keys found in code with the messages already stored:

| Situation                                              | Result                                                         |
|--------------------------------------------------------|----------------------------------------------------------------|
| Key in code, not in the locale                         | Added with a placeholder value, e.g. `welcome = welcome{ $name }`. |
| Key in code and in the locale, same variables          | Left as it is.                                                 |
| Key in code and in the locale, different variables     | The old message is commented out and a new placeholder is written. |
| Key in the locale, not in code                         | Commented out.                                                 |
| Key in the locale, but in a different file than code says | Commented out in the old file and added to the expected one. |

New keys are appended to their file in alphabetical order. Keys already in a file keep their position, and terms and
comments stay where they are.

A message referenced by another message (`{ other-message }`) counts as used, even when Python code does not call it.
A message that references a message or term that does not exist aborts the run.

With `--comment-keys-mode warn`, nothing is commented out: unused keys and variable mismatches are reported as warnings
instead. To exempt a single message, use an [ignore marker](../concepts/ignore-markers.md).

## Errors that stop the run

`ftl extract` writes nothing when any of these happen:

- A `.ftl` file has a syntax error. `ftl check --check syntax` shows the file, line and column.
- A Python file cannot be read or parsed. Keys used in that file would otherwise look unused and be commented out.
  Pass `--allow-parse-errors` to skip such files with a warning.
- The same key is used with different `_path=` values, or with different keyword arguments. The error names both call
  sites:

  ```
  [ERROR cli] Error during extraction: Extraction aborted: 1 problem found in Python sources, no .ftl files were written.
        - [key-message-conflict] Fluent key welcome is used with different keyword arguments: name and user (app/a.py:1:1, app/b.py:1:1)
  ```

## Options

`-l`, `--language <CODE>`
:   A locale to create or update. Repeat for several locales. Default: `en`.

`-k`, `--i18n-keys <NAME>`
:   Names that start a translation call. Replaces the default list: `i18n`, `L`, `LazyProxy`, `LazyFilter`.

`-K`, `--i18n-keys-append <NAME>`
:   Add a name to the list instead of replacing it, e.g. `-K LF`.

`-p`, `--i18n-keys-prefix <NAME>`
:   Objects that hold the i18n name as an attribute. With `-p self`, `self.i18n.get("key")` and `self.i18n.key()` are
    recognized. See [How keys are found](../concepts/keys.md#prefixes).

`-e`, `--exclude-dirs <GLOB>`
:   Paths to skip. Replaces the default list: `**/.venv/**`, `**/venv/**`, `**/.git/**`, `**/__pycache__/**`,
    `**/.pytest_cache/**`. Files ignored by `.gitignore` are always skipped.

`-E`, `--exclude-dirs-append <GLOB>`
:   Add a path to skip, e.g. `-E './tests/*'`.

`-i`, `--ignore-attributes <NAME>`
:   Method names that are not keys, such as `i18n.set_locale()`. Replaces the default list: `set_locale`,
    `use_locale`, `use_context`, `set_context`.

`-I`, `--append-ignore-attributes <NAME>`
:   Add a method name to ignore, e.g. `-I core` for `i18n.core.something()`.

`--ignore-kwargs <NAME>`
:   Keyword arguments that are not Fluent variables, e.g. `--ignore-kwargs when` for
    `aiogram_dialog.I18nFormat("key", when="flag")`.

`--default-ftl-file <PATH>`
:   File for keys without `_path=`. Default: `_default.ftl`.

`--comment-keys-mode <MODE>`
:   `comment` (default) comments out unused and mismatched messages; `warn` only reports them.

`--line-endings <MODE>`
:   Line endings of written files: `default` (the platform's), `lf`, `cr` or `crlf`.

`--dry-run`
:   Do everything except writing files.

`--cache`
:   Reuse the keys found in a Python file while its metadata and the extractor options are unchanged. The cache is
    stored in `.ftl-extract-cache/extract-<version>-v<schema>.bin`. Files that fail to parse are never cached.

`--cache-path <PATH>`
:   A directory or file for the cache. Enables the cache.

`--clear-cache`
:   Delete the cache before running. Enables the cache.

`--allow-parse-errors`
:   Skip Python files that cannot be read or parsed instead of aborting. Conflicting key usage still aborts.

`--leak-memory`
:   Do not free the parsed entries before exiting. On large projects freeing them takes about an eighth of the run;
    the operating system reclaims the memory when `ftl` ends anyway. The written files are the same either way.

## Example

```shell
ftl extract app/bot app/bot/locales \
  -l en -l uk \
  -K LF \
  -I core \
  -E './tests/*' \
  --ignore-kwargs when \
  --cache \
  --verbose
```
