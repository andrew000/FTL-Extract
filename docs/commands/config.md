# `ftl config`

## `ftl config sample`

```
ftl config sample [--command <extract|stub|check>]
```

Prints a ready-to-edit `pyproject.toml` configuration. Without `--command`, all three tables are printed:

```shell
ftl config sample >> pyproject.toml
```

```toml
[tool.ftl-extract.stub]
locales-path = "app/bot/locales/en"
stub-path = "app/bot/stub.pyi"
export-tree = false
```

Every key is described on the [Configuration](../configuration.md) page.
