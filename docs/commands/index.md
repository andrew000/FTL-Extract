# Commands

```
ftl [--config <PATH>] [-v] <COMMAND>
```

| Command                    | What it does                                               |
|----------------------------|------------------------------------------------------------|
| [`ftl extract`](extract.md) | Find keys in Python code and update the `.ftl` files.      |
| [`ftl stub`](stub.md)       | Generate a `.pyi` stub from the `.ftl` files of one locale. |
| [`ftl check`](check.md)     | Validate translations against each other and the code.     |
| [`ftl config`](config.md)   | Print a sample `pyproject.toml` configuration.             |

## Global options

`--config <PATH>`
:   The `pyproject.toml` to read. Without it, `ftl` searches the current directory and its parents.

`-v`, `--verbose`
:   Print debug output: every file read and written, keys kept by ignore markers, keys called with `**kwargs`.

`-V`, `--version`
:   Print the version.

## Exit codes

| Code | Meaning                                                                                              |
|------|------------------------------------------------------------------------------------------------------|
| `0`  | Success. For `ftl check`, no diagnostic reached the `fail-on` severity.                              |
| `1`  | The command ran and failed: `ftl check` found problems, a file could not be parsed or written, or `pyproject.toml` is invalid TOML. |
| `2`  | Usage or configuration error: a required path is missing, an option has an unknown value, or a locale does not exist. |
