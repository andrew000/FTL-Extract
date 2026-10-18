# How keys are found

`ftl extract` and `ftl check` parse every Python file and look for calls that start with one of the
[i18n names](../configuration.md#toolftl-extractextract). By default those are `i18n`, `L`, `LazyProxy` and
`LazyFilter`.

## Recognized calls

| Python                                   | Key                   |
|------------------------------------------|-----------------------|
| `i18n.get("welcome")`                    | `welcome`             |
| `i18n("welcome")`, `L("welcome")`        | `welcome`             |
| `LazyProxy("welcome")`                   | `welcome`             |
| `i18n.menu.title()`                      | `menu-title`          |
| `L.menu.title()`                         | `menu-title`          |
| `i18n.user.profile_title()`              | `user-profile_title`  |

An attribute chain is joined with `-`. Underscores are kept as they are, so `i18n.profile_title()` is the key
`profile_title`, not `profile-title`. Write the key with `i18n.get("profile-title")` if it contains a dash inside one
segment.

The key must be a plain string literal. These calls are skipped without a warning:

```python
i18n.get(name)              # a variable
i18n.get(f"status-{kind}")  # an f-string
getattr(i18n, attr)()       # dynamic attribute
```

Messages used only this way look unused, so `ftl extract` would comment them out. Mark them with
`# ftl-extract: ignore stale`; see [Ignore markers](ignore-markers.md).

## Other names

Add your own names with `-K` / `i18n-keys-append`, for example for
[aiogram-dialog](https://github.com/Tishka17/aiogram_dialog)'s `I18nFormat`:

```toml
[tool.ftl-extract.extract]
i18n-keys-append = ["I18nFormat"]
ignore-kwargs = ["when"]
```

```python
I18nFormat("items-count", when="has_items", n=1)  # key `items-count` with variable `n`
```

`ignore-kwargs` drops keyword arguments that are options of the call rather than Fluent variables.

## Prefixes

When the i18n object is an attribute, such as `self.i18n`, add the object's name as a prefix:

```shell
ftl extract app app/locales -p self
```

```python
self.i18n.get("welcome")   # key `welcome`
self.i18n.menu.title()     # key `menu-title`
```

## Ignored methods

Some methods of the i18n object are not keys. The first attribute after the i18n name is compared with the
[ignore-attributes](../configuration.md#toolftl-extractextract) list, which by default holds `set_locale`,
`use_locale`, `use_context` and `set_context`:

```python
i18n.set_locale("uk")      # not a key
i18n.core.something()      # not a key with `-I core`
```

## Which files are scanned

Every `.py` file under the code path, except:

- paths matching [exclude-dirs](../configuration.md#toolftl-extractextract) (`.venv`, `venv`, `.git`,
  `__pycache__` and `.pytest_cache` by default);
- files ignored by `.gitignore`, even outside a Git repository.

The code path can also be a single file.
