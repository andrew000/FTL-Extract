# Getting started

This page walks through a small bot project: extracting keys, translating them, generating a stub, and checking the
result. The examples use [aiogram-i18n](https://github.com/aiogram/i18n), but any library that calls
`i18n.get("key")` or `i18n.key()` works the same way.

## 1. A project with translatable strings

```
app/
├── handlers.py
└── locales/
```

```python title="app/handlers.py"
from aiogram_i18n import I18nContext, LazyProxy


async def start(i18n: I18nContext, name: str) -> str:
    return i18n.get("welcome", name=name)


async def menu(i18n: I18nContext) -> str:
    return i18n.menu.title()


BUTTON = LazyProxy("button-help", _path="buttons.ftl")
```

The code uses three keys:

- `welcome`, with one variable, `name`;
- `menu-title`, written as an attribute chain (`i18n.menu.title()` becomes `menu-title`);
- `button-help`, which should live in its own file, `buttons.ftl`.

## 2. Extract the keys

```shell
ftl extract app app/locales -l en -l uk
```

```
[INFO  cli] Extraction statistics:
[INFO  cli]   - Py files count: 1
[INFO  cli]   - FTL keys in code: 3
[INFO  cli]   - FTL keys added: {"uk": 3, "en": 3}
[INFO  cli] ✅ Done in 0.008s.
```

`ftl extract` creates one directory per locale and writes every key with a placeholder value:

```
app/locales/
├── en/
│   ├── _default.ftl
│   └── buttons.ftl
└── uk/
    ├── _default.ftl
    └── buttons.ftl
```

```ftl title="app/locales/en/_default.ftl"
menu-title = menu-title
welcome = welcome{ $name }
```

```ftl title="app/locales/en/buttons.ftl"
button-help = button-help
```

A placeholder is the key itself, followed by the variables the code passes. Keys without `_path=` go to
`_default.ftl`.

## 3. Translate

Replace the placeholders with real text:

```ftl title="app/locales/en/_default.ftl"
menu-title = Main menu
welcome = Welcome, { $name }!
```

```ftl title="app/locales/en/buttons.ftl"
button-help = Help
```

```ftl title="app/locales/uk/_default.ftl"
menu-title = Головне меню
welcome = Вітаємо, { $name }!
```

Leave `uk/buttons.ftl` untranslated for now; `ftl check` will point it out in step 5.

Run `ftl extract` again whenever the code changes. Translations that still match the code are never touched. New
keys are appended, and keys the code stopped using are commented out:

```ftl
menu-title = Main menu
welcome = Welcome, { $name }!

# old-promo = Summer sale
```

!!! warning "Changed variables replace the translation"

    When the variables in code no longer match a translation, `ftl extract` comments the translation out and writes
    a new placeholder. If `uk/_default.ftl` said `welcome = Вітаємо, { $user }!` while the code passes `name=`, the
    file becomes:

    ```ftl
    # welcome = Вітаємо, { $user }!

    welcome = welcome{ $name }
    ```

    The old text stays in the file as a comment, so you can copy it back with the new variable name. Use
    `--comment-keys-mode warn` to get warnings instead of edits, or an
    [ignore marker](concepts/ignore-markers.md) for messages whose variables come from somewhere the extractor cannot
    see.

## 4. Generate a stub

```shell
ftl stub app/locales/en app/stub.pyi
```

The stub gives editors and type checkers the shape of `i18n`:

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

See [ftl stub](commands/stub.md) for how to use it in your code.

## 5. Check the translations

```shell
ftl check app/locales --code-path app --suggest-from en
```

```
FTL check passed with warnings: 1 problem

Locale: uk

warn[untranslated]: key `button-help` is untranslated in locale `uk`
  file: uk/buttons.ftl:1
  suggestion[en]: button-help = Help

Summary:
- Errors: 0
- Warnings: 1
- Checks:
  - kwargs: passed
  - missing: passed
  - references: passed
  - stale: passed
  - syntax: passed
  - untranslated: failed (0 errors, 1 warnings)
```

`ftl check` exits with `1` when it finds errors, so it can run in CI. Untranslated and unused keys are warnings by
default; see [Severities](commands/check.md#severities).

## 6. Move the options to `pyproject.toml`

Instead of repeating paths and flags, put them in `pyproject.toml`:

```toml title="pyproject.toml"
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

Now the commands need no arguments:

```shell
ftl extract
ftl stub
ftl check
```

Every key is listed on the [Configuration](configuration.md) page. To keep translations checked on every pull
request, see [CI integration](ci.md).
