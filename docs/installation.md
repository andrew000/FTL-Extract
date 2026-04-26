# Installation

Install FTL-Extract with `pip`:

```shell
pip install FTL-Extract
```

Or add it with `uv`:

```shell
uv add --dev FTL-Extract
```

After installation, the `ftl` command should be available:

```shell
ftl --help
```

## Development

For local development, install dependencies with:

```shell
just sync
```

Run tests with:

```shell
cargo test --workspace
```

Generate coverage HTML with:

```shell
just test
```
