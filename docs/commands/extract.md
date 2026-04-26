# `ftl extract`

Extract Fluent keys from Python code and write them to locale `.ftl` files.

```shell
ftl extract <code-path> <output-path>
```

Example:

```shell
ftl extract app/bot app/bot/locales -l en -l uk
```

## Arguments

- `code-path`: Python source directory or file to scan.
- `output-path`: locales root directory.

## Options

- `-l`, `--language`: locale code to extract. Can be passed multiple times.
- `-k`, `--i18n-keys`: replace default i18n keys.
- `-K`, `--i18n-keys-append`: append additional i18n keys.
- `-p`, `--i18n-keys-prefix`: add prefixes such as `self.i18n`.
- `-e`, `--exclude-dirs`: replace default excluded directories.
- `-E`, `--exclude-dirs-append`: append excluded directories.
- `-i`, `--ignore-attributes`: replace ignored attributes.
- `-I`, `--append-ignore-attributes`: append ignored attributes.
- `--ignore-kwargs`: ignore keyword arguments when extracting placeholders.
- `--comment-junks`: comment invalid Fluent junk entries.
- `--default-ftl-file`: default output file name.
- `--comment-keys-mode`: handle obsolete keys with `comment` or `warn`.
- `--line-endings`: output line endings.
- `--dry-run`: run without writing files.
- `--cache`: cache Python extraction results.
- `--cache-path`: custom cache directory or file path.
- `--clear-cache`: clear extraction cache before running.
