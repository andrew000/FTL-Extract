# `ftl stub`

Generate a Python `.pyi` stub from Fluent locale files.

```shell
ftl stub <ftl-path> <output-path>
```

Example:

```shell
ftl stub app/bot/locales/en app/bot/stub.pyi
```

## Arguments

- `ftl-path`: directory containing `.ftl` files for one locale.
- `output-path`: path where the generated `stub.pyi` file is written.

## Options

- `--export-tree`: write the intermediate tree structure as JSON next to the stub.
