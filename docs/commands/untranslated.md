# `ftl untranslated`

Detect locale messages that are still placeholders, such as `hello = hello`.

```shell
ftl untranslated <locales-path>
```

Example:

```shell
ftl untranslated app/bot/locales --suggest-from en
```

## Arguments

- `locales-path`: root directory containing locale folders such as `en`, `uk`, or `pl`.

## Options

- `-l`, `--language`: check only selected locales. Can be passed multiple times.
- `--suggest-from`: use selected locales to suggest non-placeholder values.
- `--fail-on-untranslated`: exit with code `1` when untranslated keys are found.
- `--output`: write a report to a file.
- `--output-format`: report format, `txt` or `json`.

## Ignore marker

If a key intentionally has the same value as its message id, add this comment above it:

```ftl
# ftl-extract: ignore-untranslated
balance = balance
```
