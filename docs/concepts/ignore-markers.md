# Ignore markers

Some messages are intentional exceptions: brand names that must stay equal to their key, or keys built dynamically in
Python, which the extractor cannot see. A comment marker directly above a message opts it out of specific checks:

```ftl
# ftl-extract: ignore untranslated
balance = balance

# ftl-extract: ignore stale
dynamic-key = Built from an f-string in Python

# ftl-extract: ignore all
brand = brand
```

!!! note

    The marker must be directly above the message, with no blank line in between. Separated by a blank line, it is a
    standalone comment and has no effect.

## Syntax

`# ftl-extract: ignore` followed by the checks to skip, separated by commas or spaces:

| Name           | `ftl check`                                       | `ftl extract`                                      |
|----------------|---------------------------------------------------|----------------------------------------------------|
| `stale`        | Not reported as unused.                           | Kept instead of being commented out.               |
| `untranslated` | Not reported when equal to its key.               | —                                                  |
| `kwargs`       | Left out of the `kwargs` check.                   | Kept when its variables differ from the code.      |
| `all`          | All of the above.                                 | All of the above.                                  |

A bare `# ftl-extract: ignore` means `all`. Each command acts on the names it knows and skips the rest.

A message ignored for `stale` also keeps the messages and terms it references alive, in both commands, as if Python
code used it.

## Dynamic keys

```python
i18n.get(f"status-{kind}")
```

```ftl
# ftl-extract: ignore stale
status-ok = OK

# ftl-extract: ignore stale
status-failed = Failed
```

## Variables the extractor cannot see

```python
i18n.get("items")  # `count` is added by a template or a middleware
```

```ftl
# ftl-extract: ignore kwargs
items = You have { $count } items
```

A call with `**kwargs` needs no marker; see [Variables and kwargs](kwargs.md#calls-with-kwargs).

## Notes

- `ignore stale` does not cover a kwargs mismatch, and `ignore kwargs` does not cover an unused key. `ignore all`
  covers both.
- A message kept by a marker is checked like a used one, so a reference to a message or term that does not exist
  still aborts `ftl extract`.
- Run `ftl extract --verbose` to list kept keys: `key "status-ok" is kept: marker ignores stale`.
- The older spelling `# ftl-extract: ignore-untranslated`, and a bare `# ignore` line, still work as aliases of
  `# ftl-extract: ignore untranslated`.
