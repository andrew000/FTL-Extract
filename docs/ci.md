# CI integration

Keep the configuration in `pyproject.toml` (see [Configuration](configuration.md)) so the CI commands need no
arguments.

## GitHub Actions

```yaml title=".github/workflows/translations.yml"
name: Translations

on:
  push:
  pull_request:

jobs:
  ftl:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: astral-sh/setup-uv@v10

      - name: Validate translations
        run: uvx --from FTL-Extract ftl check

      - name: Check that the .ftl files are up to date
        run: |
          uvx --from FTL-Extract ftl extract
          git diff --exit-code
```

The first step fails when [`ftl check`](commands/check.md) finds errors. The second fails when someone changed the code
without running `ftl extract`: the run rewrites the `.ftl` files, and `git diff --exit-code` exits with `1` on any
change.

If FTL-Extract is a dev dependency of the project, use `uv run ftl check` instead of `uvx`, so CI uses the version
from your lock file.

To keep the report as a build artifact:

```yaml
      - name: Validate translations
        run: uvx --from FTL-Extract ftl check --report-path reports/ftl-check

      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: ftl-check
          path: reports/ftl-check.json
```

## pre-commit

```yaml title=".pre-commit-config.yaml"
repos:
  - repo: local
    hooks:
      - id: ftl-extract
        name: ftl extract
        entry: ftl extract
        language: python
        additional_dependencies: ["FTL-Extract==0.12.2"]
        pass_filenames: false
        files: \.(py|ftl)$

      - id: ftl-check
        name: ftl check
        entry: ftl check
        language: python
        additional_dependencies: ["FTL-Extract==0.12.2"]
        pass_filenames: false
        files: \.(py|ftl)$
```

`pass_filenames: false` is required: both commands read their paths from `pyproject.toml` and always look at the
whole project. When `ftl extract` changes a file, pre-commit fails the commit so you can review and stage the result.

Add `--fail-on warn` to the `ftl check` entry if stale and untranslated keys should block commits too.
