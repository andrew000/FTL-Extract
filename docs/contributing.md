# Contributing

Pull requests are welcome. For larger changes, please open an [issue](https://github.com/andrew000/FTL-Extract/issues)
first to discuss what you would like to change.

## Repository layout

The `ftl` binary is a Rust workspace packaged as a Python wheel:

| Crate           | Contents                                       |
|-----------------|------------------------------------------------|
| `src/cli`       | The `ftl` binary: arguments and configuration. |
| `src/extractor` | Key extraction from Python and `.ftl` updates. |
| `src/stub`      | `.pyi` stub generation.                        |
| `src/check`     | The `ftl check` validations and reports.       |
| `src/common`    | Code shared by the crates above.               |

## Development commands

The [just](https://github.com/casey/just) recipes cover the usual tasks:

| Command                  | What it does                                         |
|--------------------------|------------------------------------------------------|
| `just sync`              | Install the Python development tools with uv.        |
| `just format`            | `cargo fix` and `cargo fmt`.                         |
| `just lint`              | `cargo clippy` on all targets.                       |
| `just test`              | Tests with an HTML coverage report.                  |
| `just bench`             | Extractor benchmarks.                                |
| `just bench-cmp baseline`| Compare the benchmarks with a saved baseline.        |
| `just build`             | Build the wheel and the sdist.                       |
| `just docs`              | Serve this documentation with live reload.           |
| `just docs-build`        | Build the documentation, failing on any warning.     |

For a quick correctness check, run:

```shell
cargo test --workspace
```

## Pull requests

- Add tests next to the code you change.
- Include benchmark results for performance work.
- Update the documentation in `docs/` and add an entry to `CHANGELOG.md` for user-visible changes.
- Run the pre-commit hooks before pushing.
