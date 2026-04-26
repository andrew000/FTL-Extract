# Configuration

FTL-Extract can read command defaults from `pyproject.toml`.

Use command-specific tables:

```toml
[tool.ftl-extract.extract]
code-path = "app/bot"
output-path = "app/bot/locales"
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
ftl-path = "app/bot/locales/en"
output-path = "app/bot/stub.pyi"
export-tree = false

[tool.ftl-extract.untranslated]
locales-path = "app/bot/locales"
languages = ["uk"]
suggest-from = ["en"]
fail-on-untranslated = true
output = "reports/untranslated"
output-format = "json"
```

Then run:

```shell
ftl extract
ftl stub
ftl untranslated
```

## Config discovery

By default, `ftl` searches for `pyproject.toml` from the current directory upward.

Use `--config` to select a specific file:

```shell
ftl --config ./pyproject.toml extract
```

CLI arguments override values from `pyproject.toml`.

## Sample config

Print a ready-to-edit sample:

```shell
ftl config sample
```

Print one command section:

```shell
ftl config sample --command extract
```
