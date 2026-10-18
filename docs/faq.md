# FAQ

## How do I add a language?

Pass it to `ftl extract`, together with the existing ones:

```shell
ftl extract app app/locales -l en -l uk -l pl
```

or add it to `languages` in `[tool.ftl-extract.extract]`. The new locale gets every key with a placeholder value.

## How do I detect names like `LF` or `I18nFormat`?

Append them to the default list:

```shell
ftl extract app app/locales -K LF -K I18nFormat
```

`-k` replaces the list instead of extending it. See [How keys are found](concepts/keys.md#other-names).

## Why did my translation turn into a comment?

Either the code no longer uses the key, or it passes different variables than the translation expects. In both cases
`ftl extract` comments the old message out instead of deleting it. See
[What changes in the .ftl files](commands/extract.md#what-changes-in-the-ftl-files).

To get warnings instead, use `--comment-keys-mode warn`. To keep a single message, use an
[ignore marker](concepts/ignore-markers.md).

## Why is a key reported as both missing and stale?

The key lives in a different file than the code says. See
[Keys are matched per file](concepts/file-layout.md#keys-are-matched-per-file).

## Why is a key called with `**kwargs` not checked?

`**kwargs` can pass any variable, so the extractor cannot know which ones the call provides. See
[Calls with **kwargs](concepts/kwargs.md#calls-with-kwargs).

## Why are keys built with f-strings reported as stale?

The extractor only sees string literals. Mark such messages with `# ftl-extract: ignore stale`; see
[Dynamic keys](concepts/ignore-markers.md#dynamic-keys).

## `ftl extract` refuses to run because of a Python syntax error

A file that cannot be parsed would make all of its keys look unused, and `ftl extract` would comment them out. Fix
the file, exclude it with `-E`, or pass `--allow-parse-errors` to skip it with a warning.

## Does the cache need to be committed?

No. Add `.ftl-extract-cache/` to `.gitignore`. The cache file is named after the FTL-Extract version, so upgrading
starts a new one.

## How do I see what `ftl extract` is doing?

Run it with `--verbose`. Add `--dry-run` to see the result without writing files.
