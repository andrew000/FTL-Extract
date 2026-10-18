# Variables and kwargs

The keyword arguments of a call are the Fluent variables the code provides:

```python
i18n.get("welcome", name=user.name)
```

```ftl
welcome = Welcome, { $name }!
```

`_path` and the names in [ignore-kwargs](../configuration.md#toolftl-extractextract) are not variables.

## When code and translation disagree

If a message uses different variables than the code passes, the translation would fail or show the wrong text at
runtime.

- `ftl check --check kwargs` reports it as an error, listing the variables on each side:

    ```
    error[kwargs]: key `welcome` in locale `uk` has variable mismatch
      file: uk/_default.ftl:2
      code: app/handlers.py:5:12
      missing in code: user
      unused in ftl: name
    ```

- `ftl extract` comments the old translation out and writes a placeholder with the new variables:

    ```ftl
    # welcome = Вітаємо, { $user }!

    welcome = welcome{ $name }
    ```

    With `--comment-keys-mode warn` it only reports the mismatch.

If a variable is supplied by something the extractor cannot see, mark the message with
`# ftl-extract: ignore kwargs`; see [Ignore markers](ignore-markers.md).

## The same key in several places

All calls of a key must pass the same keyword arguments, in any order. Otherwise `ftl extract` aborts before writing
anything:

```
- [key-message-conflict] Fluent key welcome is used with different keyword arguments: name and user (app/a.py:1:1, app/b.py:1:1)
```

## Calls with `**kwargs`

```python
i18n.get("welcome", **data)
```

`**data` can pass any keyword argument, so the variables of such a key cannot be verified:

- `ftl extract` leaves the stored translation as it is. A new key is still written with the explicit keyword arguments
  it saw, e.g. `welcome = welcome`.
- `ftl check --check kwargs` skips the key.
- Calls with `**` never conflict with other calls. The placeholder for a new key takes its variables from the first
  call without `**`, or from the first call when every call uses `**`.

Run with `--verbose` to see which keys are affected:

```
key "welcome" is called with **kwargs at app/a.py:3:5; its variables cannot be verified
```
