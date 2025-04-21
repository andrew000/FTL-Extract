# FTL-Extract

## Description

FTL-Extract is a Python package that extracts Fluent keys from .py files and generates a .ftl file with extracted keys.

***

## Installation

Use the package manager [pip](https://pip.pypa.io/en/stable) to install FTL-Extract.

```shell
$ pip install FTL-Extract
```

Or use modern tool like [UV](https://docs.astral.sh/uv/) to install FTL-Extract.

```shell
$ uv add --dev FTL-Extract
```

***

## Usage

First of all, you should create locales directory in your project.

```shell
$ mkdir project_path/locales
```

Then, you can use the following command to extract keys from your code.

```shell
$ ftl_extract project_path/code_path project_path/locales
```

By default, FTL-Extract will create a directory named `en` and put all keys into `_default.ftl` file.

In some cases, you may want to extract keys to specific `.ftl` files.
So, there is new keyword argument `_path` in `i18n.get` and `i18n.<key>`.

```python
# Before
i18n.get("key-1", arg1="value1", arg2="value2")

# After
i18n.get("key-1", arg1="value1", arg2="value2", _path="dir/ftl_file.ftl")

# Also
i18n.key_1(arg1="value1", arg2="value2", _path="dir/ftl_file.ftl")

# Or
i18n.some.key_1(arg1="value1", arg2="value2", _path="dir/ftl_file.ftl")
```

***

## 💁‍♂️ Explanation of the `ftl-extractor` command

```shell
$ ftl_extract project_path/code_path project_path/locales
```

- `project_path/code_path` - path to the project directory where the code is located.
- `project_path/locales` - path to the project directory where the `.ftl` files will be located.

### 📚 Additional arguments

- `-l` or `--language` - add a new language to the project.
- `-k` or `--i18n_keys` - add additional i18n keys to the extractor.
- `--ignore-attributes` - ignore specific attributes of the `i18n.*` like `i18n.set_locale`.
- `-a` or `--expand-ignore-attributes` - add more attributes to ignore to the default list.
- `--ignore-kwargs` - ignore specific kwargs of the i18n_keys like `when=...` in
  `aiogram_dialog.I18nFormat(..., when=...)`.
- `--comment-junks` - comments errored translations in the `.ftl` file.
- `--default-ftl-file` - specify the default `.ftl` file name.
- `--comment-keys-mode` - specify the comment keys mode. It will comment keys that are not used in the code or print
  warnings about them. Available modes: `comment`, `warn`.
- `--dry-run` - run the command without making any changes to the files.
- `-v` or `--verbose` - print additional information about the process.

***

## FAQ

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

***

## How I use FTL-Extract in most of my projects

```shell
$ ftl_extract \
  './app/bot' \
  './app/bot/locales' \
  -l 'en' \
  -l 'uk' \
  -k 'i18n' \
  -k 'L' \
  -k 'LF' \
  -k 'LazyProxy' \
  -a 'core' \
  --comment-junks \
  --comment-keys-mode 'warn' \
  --verbose
```

***

## Contributing

Pull requests are welcome. For major changes, please open an issue first
to discuss what you would like to change.

Please make sure to update tests as appropriate.
