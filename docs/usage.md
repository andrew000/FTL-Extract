# Usage

Create a locales directory in your project:

```shell
mkdir app/bot/locales
```

Extract keys from Python code:

```shell
ftl extract app/bot app/bot/locales
```

By default, FTL-Extract creates an `en` locale and writes extracted messages to `_default.ftl`.

## Targeting a specific FTL file

Use `_path` in supported i18n calls:

```python
i18n.get("key-1", arg1="value1", _path="pages/main.ftl")
i18n.key_1(arg1="value1", _path="pages/main.ftl")
i18n.some.key_1(arg1="value1", _path="pages/main.ftl")
```

## Common command

```shell
ftl extract \
  app/bot \
  app/bot/locales \
  -l en \
  -l uk \
  -K LF \
  -I core \
  -E "./tests/*" \
  --ignore-kwargs when \
  --comment-junks \
  --comment-keys-mode comment \
  --verbose
```
