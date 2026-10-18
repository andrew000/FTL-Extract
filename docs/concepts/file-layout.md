# File layout and `_path`

Each locale is a directory under the locales path, and each key lives in exactly one `.ftl` file inside it. By default
that file is `_default.ftl`; change it with `default-ftl-file`.

## Choosing a file with `_path`

Pass `_path=` in the call to put a key in another file. The path is relative to the locale directory and is the same
for every locale:

```python
i18n.get("page-title", _path="pages/main.ftl")    # en/pages/main.ftl, uk/pages/main.ftl
i18n.get("page-footer", _path="pages")            # en/pages/_default.ftl
i18n.menu.title(_path="menu.ftl")                 # en/menu.ftl
LazyProxy("button-help", _path="buttons.ftl")     # en/buttons.ftl
```

A `_path` without an extension is a directory, and the key goes to its default file. `_path` must be a string
literal; it is never treated as a Fluent variable.

At runtime `_path` is still passed to the call like any other keyword argument, so make sure your i18n library
tolerates it.

A key used with two different `_path` values aborts `ftl extract`:

```
- [key-path-conflict] Fluent key x has different paths: a.ftl and b.ftl (app/b.py:1:1, app/b.py:2:1)
```

## Keys are matched per file

The `missing`, `stale` and `kwargs` checks compare a key together with its file. A key that exists in the locale but
lives in a different file than the code declares is reported twice: as `missing` in the file the code expects, and as
`stale` in the file where it is.

```python
i18n.get("page-title", _path="pages/main.ftl")
```

```ftl title="locales/en/_default.ftl"
# missing in pages/main.ftl, stale in _default.ftl
page-title = Page title
```

This is intentional: `ftl extract` owns the layout and always writes a key to the file named by `_path` (or the
default file). It surprises people who organized their files by hand. Either add the matching `_path=` in code, or
run `ftl extract`, which moves the key to the expected file and comments out the old copy.
