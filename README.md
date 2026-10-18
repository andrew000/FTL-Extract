# FTL-Extract

**FTL-Extract** keeps the [Fluent](https://projectfluent.org/) translation files of a Python project in sync with the
code that uses them. It finds every translation key in your `.py` files and writes the `.ftl` files for each locale:
new keys are added, keys the code no longer uses are commented out, and existing translations are left alone.

The `ftl` CLI is written in Rust and ships as a native binary inside the Python wheel.

📖 **Documentation: <https://ftl-extract.readthedocs.io/>**

## Installation

```shell
uv add --dev FTL-Extract
# or
pip install FTL-Extract
```

## Quick start

```python
# app/handlers.py
async def start(i18n, name: str) -> str:
    return i18n.get("welcome", name=name)


async def menu(i18n) -> str:
    return i18n.menu.title()
```

```shell
ftl extract app app/locales -l en -l uk
```

```ftl
# app/locales/en/_default.ftl
menu-title = menu-title
welcome = welcome{ $name }
```

Translate the placeholders, then keep everything in sync:

```shell
ftl extract app app/locales -l en -l uk          # update .ftl files after code changes
ftl stub app/locales/en app/stub.pyi             # generate a .pyi stub for autocompletion
ftl check app/locales --code-path app            # validate translations in CI
```

The options can live in `pyproject.toml`, so the commands need no arguments:

```toml
[tool.ftl-extract.extract]
code-path = "app"
locales-path = "app/locales"
languages = ["en", "uk"]

[tool.ftl-extract.stub]
locales-path = "app/locales/en"
stub-path = "app/stub.pyi"

[tool.ftl-extract.check]
locales-path = "app/locales"
code-path = "app"
```

## Learn more

- [Getting started](https://ftl-extract.readthedocs.io/en/latest/getting-started/)
- [Configuration reference](https://ftl-extract.readthedocs.io/en/latest/configuration/)
- [`ftl extract`](https://ftl-extract.readthedocs.io/en/latest/commands/extract/),
  [`ftl stub`](https://ftl-extract.readthedocs.io/en/latest/commands/stub/),
  [`ftl check`](https://ftl-extract.readthedocs.io/en/latest/commands/check/)
- [CI integration](https://ftl-extract.readthedocs.io/en/latest/ci/)
- [FAQ](https://ftl-extract.readthedocs.io/en/latest/faq/)
- [Changelog](https://github.com/andrew000/FTL-Extract/blob/master/CHANGELOG.md)

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.
See the [contributing guide](https://ftl-extract.readthedocs.io/en/latest/contributing/).
