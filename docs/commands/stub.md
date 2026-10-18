# `ftl stub`

```
ftl stub [OPTIONS] [LOCALES_PATH] [STUB_PATH]
```

Reads every `.ftl` file under `LOCALES_PATH` (one locale, including subdirectories) and writes a `.pyi` stub to
`STUB_PATH`. With the stub, editors autocomplete `i18n.menu.title()`, and type checkers know which keyword arguments
each message takes.

```shell
ftl stub app/locales/en app/stub.pyi
```

Both paths can come from [`[tool.ftl-extract.stub]`](../configuration.md#toolftl-extractstub).

## What is generated

Message keys are split on `-` into nested namespaces. Each message becomes a static method whose keyword-only
parameters are the message's variables, and whose return type is the translation text:

```ftl title="app/locales/en/_default.ftl"
menu-title = Main menu
welcome = Welcome, { $name }!
```

```python title="app/stub.pyi (excerpt)"
class I18nStub:
    class __Menu:
        @staticmethod
        def title(**kwargs: Any) -> Literal['Main menu']:
            ...

    menu = __Menu
    @staticmethod
    def welcome(*, name: Any, **kwargs: Any) -> Literal['Welcome, { $name }!']:
        ...
```

Variables of the messages a message references are parameters too, because Fluent formats a referenced message with
the caller's arguments. For `greet = { hello } and { $x }` with `hello = Hi { $name }`, the stub declares
`def greet(*, x: Any, name: Any, **kwargs: Any)`. Variables inside a term are not added: a term only sees the
arguments of its own call, such as `{ -brand(case: "gen") }`.

When a key is both a message and a prefix of other keys (`menu` and `menu-title`), the stub declares both an
`@overload` method and a namespace class under that name.

The stub also declares `I18nContext`, with the methods of
[aiogram-i18n](https://github.com/aiogram/i18n)'s context (`get`, `set_locale`, `use_locale`, `use_context`,
`set_context`), and `L: LazyFactory` for lazy strings.

## Using the stub

Import the stub classes only while type checking:

```python
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from app.stub import I18nContext
else:
    from aiogram_i18n import I18nContext


async def start(i18n: I18nContext, name: str) -> str:
    return i18n.welcome(name=name)
```

Regenerate the stub after `ftl extract` or whenever translations change. The file starts with
`# This is auto-generated file, do not edit!`.

## Options

`--export-tree`
:   Also write the key tree as JSON next to the stub (`app/stub.json` for `app/stub.pyi`). Useful for debugging or
    for feeding other tools:

    ```json
    {
      "welcome": {
        "$_meta$": { "args": ["name"], "translation": "Welcome, { $name }!" }
      }
    }
    ```

A `.ftl` file with a syntax error stops the command with exit code `1`.
