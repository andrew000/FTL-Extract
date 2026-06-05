# FTL-Extract

## Description

**FTL-Extract** is a Python package that extracts Fluent keys from `.py` files and generates `.ftl` file with extracted
keys.

The `ftl` CLI is implemented in Rust and ships as a native binary inside the Python wheel.

***

## Installation

Use the package manager [pip](https://pip.pypa.io/en/stable) to install FTL-Extract.

```shell
$ pip install FTL-Extract
```

Or use modern tool like [UV](https://docs.astral.sh/uv/) to install FTL-Extract.

```shell
$ uv add --dev FTL-Extract
```

***

## Usage

First of all, you should create locales directory in your project.

```shell
$ mkdir project_path/locales
```

Then, you can use the following command to extract keys from your code.

```shell
$ ftl extract project_path/code_path project_path/locales
```

By default, FTL-Extract will create a directory named `en` and put all keys into `_default.ftl` file.

You can also keep command defaults in `pyproject.toml`:

```toml
[tool.ftl-extract.extract]
code-path = "project_path/code_path"
locales-path = "project_path/locales"
languages = ["en", "uk"]
i18n-keys-append = ["LF", "LazyProxy"]
ignore-attributes-append = ["core"]
exclude-dirs-append = ["./tests/*"]
ignore-kwargs = ["when"]
comment-junks = true
comment-keys-mode = "comment"
line-endings = "lf"
cache = true

[tool.ftl-extract.stub]
locales-path = "project_path/locales/en"
stub-path = "project_path/code_path/stub.pyi"
export-tree = false

[tool.ftl-extract.check]
locales-path = "project_path/locales"
code-path = "project_path/code_path"
languages = ["uk"]
checks = ["all"]
suggest-from = ["en"]
fail-on = ["error"]
report-path = "reports/ftl-check"
report-format = "json"
```

Then run commands without repeating the configured paths:

```shell
$ ftl extract
$ ftl stub
$ ftl check
```

By default, `ftl` searches for `pyproject.toml` from the current directory upward. Use `--config` to select a specific
file:

```shell
$ ftl --config ./pyproject.toml extract
```

CLI arguments override values from `pyproject.toml`; built-in defaults are used when neither is provided.

To print a ready-to-edit configuration sample, use:

```shell
$ ftl config sample
$ ftl config sample --command extract
```

In some cases, you may want to extract keys to specific `.ftl` files.
So, there is new keyword argument `_path` in `i18n.get` and `i18n.<key>`.

```python
# Before
i18n.get("key-1", arg1="value1", arg2="value2")

# After
i18n.get("key-1", arg1="value1", arg2="value2", _path="dir/ftl_file.ftl")

# Also
i18n.key_1(arg1="value1", arg2="value2", _path="dir/ftl_file.ftl")

# Or
i18n.some.key_1(arg1="value1", arg2="value2", _path="dir/ftl_file.ftl")
```

***

## 💁‍♂️ Explanation of the `ftl extract` command

```shell
$ ftl extract project_path/code_path project_path/locales
```

- `project_path/code_path` - path to the project directory where the code is located.
- `project_path/locales` - path to the project directory where the `.ftl` files will be located.

### 📚 Additional arguments

- `-l` or `--language` - add a new language to the project.
- `-k` or `--i18n-keys` - add additional i18n keys to the extractor.
- `-K` or `--i18n-keys-append` - add additional i18n keys to the extractor and append them to the default list.
- `-p` or `--i18n-keys-prefix` - add a prefix to the i18n keys. For example, `self.i18n.<key>()`.
- `-e` or `--exclude-dirs` - exclude specific directories from the extraction process.
- `-E` or `--exclude-dirs-append` - add more directories to exclude from the extraction process.
- `-i` or `--ignore-attributes` - ignore specific attributes of the `i18n.*` like `i18n.set_locale`.
- `-I` or `--append-ignore-attributes` - add more attributes to ignore to the default list.
- `--ignore-kwargs` - ignore specific kwargs of the i18n_keys like `when=...` in
  `aiogram_dialog.I18nFormat(..., when=...)`.
- `--comment-junks` - comments errored translations in the `.ftl` file.
- `--default-ftl-file` - specify the default `.ftl` file name.
- `--comment-keys-mode` - specify the comment keys mode. It will comment keys that are not used in the code or print
  warnings about them. Available modes: `comment`, `warn`.
- `-v` or `--verbose` - print additional information about the process.
- `--dry-run` - run the command without making any changes to the files.
- `--cache` - cache extracted Python keys between runs and reuse them when source file metadata and extractor options
  are unchanged. By default, the cache is stored in
  `.ftl-extract-cache/extract-<package-version>-v<schema-version>.bin`.
- `--cache-path` - custom cache directory or file path. Directory paths store the cache as
  `extract-<package-version>-v<schema-version>.bin`. Passing this option enables the cache.
- `--clear-cache` - delete the existing extraction cache before running.

***

## 💁‍♂️ Explanation of the `ftl stub` command

```shell
$ ftl stub 'project_path/locales/<locale>' 'project_path/code_path'
```

- `project_path/locales/<locale>` - path to the locales directory where the `<locale>` directory (e.g. `en`) contains `.ftl` files located.
- `project_path/code_path` - path to the directory where the `stub.pyi` will be located.

***

## 💁‍♂️ Explanation of the `ftl check` command

```shell
$ ftl check project_path/locales --code-path project_path/code_path -l uk --suggest-from en
```

- `project_path/locales` - path to the locales root directory that contains locale folders like `en`, `uk`, etc.

### 📚 Additional arguments

- `--check` - validation to run. Supported checks: `all`, `untranslated`, `syntax`, `references`, `missing`, `stale`, `kwargs`. If omitted, all checks run.
- `--code-path` - path to Python code. Required for `--check missing`, `--check stale`, and `--check kwargs`.
- `-l` or `--language` - check only selected locales. Can be passed multiple times.
- `--suggest-from` - locale(s) used to suggest non-placeholder translations for missing items. Can be passed multiple times.
- `--fail-on` - minimum diagnostic severity that should return exit code `1`, for example `--fail-on error`. `--fail-on warn` also fails on errors.
- `--report-path` - optional report file path for batch processing reports. If no extension is provided, `.txt` or `.json` is appended automatically based on `--report-format`.
- `--report-format` - report file format: `terminal` or `json` (default: `json`).

In default/all mode, `ftl check` runs syntax validation first. If syntax errors are found, the remaining checks are
skipped until the Fluent files are fixed, and the command still returns a normal check report. The process exit code is
controlled by `fail-on`.

The `stale` check treats a message referenced by another `.ftl` message as used, even when Python code does not call it
directly.

Breaking change: `ftl untranslated` has been removed. Use `ftl check --check untranslated` instead.

### Config examples for each check

Run every available check. This is also the default when `checks` is omitted:

```toml
[tool.ftl-extract.check]
locales-path = "app/bot/locales"
code-path = "app/bot"
languages = ["uk", "pl"]
checks = ["all"]
suggest-from = ["en"]
fail-on = ["error"]
report-path = "reports/ftl-check"
report-format = "json"
```

Check only untranslated placeholders. This check does not need `code-path`:

```toml
[tool.ftl-extract.check]
locales-path = "app/bot/locales"
languages = ["uk", "pl"]
checks = ["untranslated"]
suggest-from = ["en"]
fail-on = ["error"]
report-format = "terminal"
```

Check only Fluent syntax errors. This check does not need `code-path`:

```toml
[tool.ftl-extract.check]
locales-path = "app/bot/locales"
languages = ["uk", "pl"]
checks = ["syntax"]
fail-on = ["error"]
report-format = "terminal"
```

Check only missing message and term references inside `.ftl` files. This check does not need `code-path`:

```toml
[tool.ftl-extract.check]
locales-path = "app/bot/locales"
languages = ["uk", "pl"]
checks = ["references"]
fail-on = ["error"]
report-format = "terminal"
```

Check keys used in Python but missing from locale files. This check requires `code-path`:

```toml
[tool.ftl-extract.check]
locales-path = "app/bot/locales"
code-path = "app/bot"
languages = ["uk", "pl"]
checks = ["missing"]
suggest-from = ["en"]
fail-on = ["error"]
report-format = "terminal"
```

Check stale `.ftl` messages that are not used by Python. This check requires `code-path`:

```toml
[tool.ftl-extract.check]
locales-path = "app/bot/locales"
code-path = "app/bot"
languages = ["uk", "pl"]
checks = ["stale"]
fail-on = ["error"]
report-format = "terminal"
```

Check Python keyword arguments against Fluent variables. This check requires `code-path`:

```toml
[tool.ftl-extract.check]
locales-path = "app/bot/locales"
code-path = "app/bot"
languages = ["uk", "pl"]
checks = ["kwargs"]
fail-on = ["error"]
report-format = "terminal"
```

Run a custom subset:

```toml
[tool.ftl-extract.check]
locales-path = "app/bot/locales"
code-path = "app/bot"
languages = ["uk", "pl"]
checks = ["syntax", "references", "missing", "kwargs"]
suggest-from = ["en"]
fail-on = ["error"]
report-path = "reports/ftl-check"
report-format = "json"
```

### 🙈 Ignore marker for intentional placeholders

If a key is intentionally the same as its message id (for example, brand or domain terms like `balance = balance`), add a message comment marker above it:

```ftl
# ftl-extract: ignore-untranslated
balance = balance
```

This key will be skipped by `ftl check --check untranslated`.


## FAQ

#### ❓ - How to add more languages to the project ?

```shell
# Here we add 3 languages: English, Ukrainian and Polish
$ ftl extract project_path/code_path project_path/locales -l en -l uk -l pl
```

#### ❓ - How to detect another i18n keys like `LazyProxy` or `L` ?

```shell
# Here we extract ftl keys from i18n-keys like `LF`, `LazyProxy` and `L`
$ ftl extract project_path/code_path project_path/locales -K LF -K LazyProxy -K L
```

***

## How I use FTL-Extract in most of my projects

```shell
$ ftl extract \
  'app/bot' \
  'app/bot/locales' \
  -l 'en' \
  -l 'uk' \
  -K 'LF' \
  -I 'core' \
  -E './tests/*' \
  --ignore-kwargs 'when' \
  --comment-junks \
  --comment-keys-mode 'comment' \
  --cache \
  --verbose
```

***

## Contributing

Pull requests are welcome. For major changes, please open an issue first
to discuss what you would like to change.

Please make sure to update tests as appropriate.
