# FTL-Extract

**FTL-Extract** keeps the [Fluent](https://projectfluent.org/) translation files of a Python project in sync with the
code that uses them. It reads your `.py` files, finds every translation key, and writes the `.ftl` files for each
locale: new keys are added, keys the code no longer uses are commented out, and existing translations are left alone.

The `ftl` command is written in Rust and ships as a native binary inside the Python wheel, so it needs no Python
dependencies at runtime and handles tens of thousands of keys in well under a second.

## What it does

`ftl extract`
:   Scans Python code for calls such as `i18n.get("welcome", name=name)` or `i18n.menu.title()` and updates the
    `.ftl` files of every locale. See [ftl extract](commands/extract.md).

`ftl stub`
:   Generates a `.pyi` stub from your `.ftl` files, so editors autocomplete `i18n.menu.title()` and type checkers
    know its arguments. See [ftl stub](commands/stub.md).

`ftl check`
:   Validates translations for CI: syntax errors, broken references, keys missing from a locale, keys no longer used,
    untranslated placeholders, and variables that do not match the code. See [ftl check](commands/check.md).

## Installation

=== "uv"

    ```shell
    uv add --dev FTL-Extract
    ```

=== "pip"

    ```shell
    pip install FTL-Extract
    ```

FTL-Extract supports Python 3.11 to 3.15. Wheels are published for Linux (manylinux and musllinux, x86_64 and
aarch64), Windows (x86_64) and macOS (arm64 and x86_64).

Check that the binary is available:

```shell
ftl --version
```

## Next steps

- [Getting started](getting-started.md) walks through a small project from the first extraction to a CI check.
- [Configuration](configuration.md) lists every `pyproject.toml` key.
- [How keys are found](concepts/keys.md) explains which Python calls are recognized.
