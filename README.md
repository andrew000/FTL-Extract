# FTL-Extract

## Description

FTL-Extract is a Python package that extracts Fluent keys from .py files and generates a .ftl file with extracted keys.

## Installation

Use the package manager [pip](https://pip.pypa.io/en/stable) to install FTL-Extract.

```shell
$ pip install FTL-Extract
```

Or add it to your `pyproject.toml` and run `poetry update`

## Usage

First of all, you should to create locales directory in your project.

```shell
$ mkdir project_path/locales
```

Then, you can use the following command to extract keys from your code.

```shell
$ ftl_extract project_path/code_path project_path/locales
```

By default, FTL-Extract will create a directory named `en` and put all keys into `_default.ftl` file.

In more cases, you may want to extract keys to specific `.ftl` files.
So, you must add `_path` argument to `i18n.get` function in your code.

```python
# Before
i18n.get("key-1", arg1="value1", arg2="value2")

# After
i18n.get("key-1", arg1="value1", arg2="value2", _path="dir/ftl_file.ftl")
```

## FAQ

#### ❓ - What changed 🤔

You just need to add `_path` argument to `i18n.get` function and specify the path to the `.ftl` file where you want to
put the key.

It may be just a filename like `file.ftl` or a path to a file like `dir/file.ftl`.

#### ❓ - My `FluentRuntimeCore` throws an error 🤯, when I use `_path` argument

Now there is a little problem with integration with [aiogram-i18n](https://github.com/aiogram/i18n)

To fix any possible problems - when you create a `i18n` middleware in your code:

```python
i18n_middleware = I18nMiddleware(
    core=FluentRuntimeCore(path=Path(__file__).parent / "locales" / "{locale}"),
    manager=FSMManager(),
)
```

you should replace `FluentRuntimeCore` with your own patched core.

_**🤖 Example of your own `Core`**_

```python
class CustomFluentRuntimeCore(FluentRuntimeCore):
    def get(self, message_id: str, locale: Optional[str] = None, /, **kwargs: Any) -> str:

    # PATCH START #
    kwargs.pop("_path", None)
    # PATCH END #

    locale = self.get_locale(locale=locale)
    translator: FluentBundle = self.get_translator(locale=locale)
    ...
```

Then just use this `CustomFluentRuntimeCore` in your `i18n` middleware as erlier.

#### ❓ - How to add more languages to the project ?

```shell
# Here we add 3 languages: English, Ukrainian and Polish
$ ftl_extract project_path/code_path project_path/locales -l en -l uk -l pl
```

#### ❓ - How to detect another i18n keys like `LazyProxy` or `L` ?

```shell
# Here we extract ftl keys from i18n-keys like `i18n`, `LF`, `LazyProxy` and `L`
$ ftl_extract project_path/code_path project_path/locales -k i18n -k LF -k LazyProxy -k L
```

## How I use FTL-Extract in most of my projects

```shell
$ ftl_extract \
  '.\app\bot' \
  '.\app\bot\locales' \
  -l 'en' \
  -l 'uk' \
  -l 'pl' \
  -l 'de' \
  -l 'ja' \
  -l 'ru' \
  -k 'i18n' \
  -k 'L' \
  -k 'LF' \
  -k 'LazyProxy'
```

## Contributing

Pull requests are welcome. For major changes, please open an issue first
to discuss what you would like to change.

Please make sure to update tests as appropriate.
