# `ftl config`

Helpers for working with FTL-Extract configuration.

## `ftl config sample`

Print a ready-to-edit `pyproject.toml` sample:

```shell
ftl config sample
```

Print only one command-specific section:

```shell
ftl config sample --command extract
ftl config sample --command stub
ftl config sample --command untranslated
```
