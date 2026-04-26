<section class="hero">
  <p class="hero__eyebrow">Python Fluent tooling</p>
  <h1>Extract and maintain Fluent keys from Python code</h1>
  <p class="hero__lead">
    FTL-Extract ships a fast Rust CLI inside a Python package. It scans Python source,
    updates locale files, generates typed stubs, and reports untranslated placeholders.
  </p>
  <div class="actions">
    <a class="md-button md-button--primary" href="installation/">Install</a>
    <a class="md-button" href="usage/">Usage</a>
  </div>
</section>

```shell
pip install FTL-Extract
ftl extract app/bot app/bot/locales -l en -l uk
```

<div class="cards">
  <a class="card" href="commands/extract/">
    <strong>Extract keys</strong>
    <span>Scan Python code and write missing Fluent messages to locale files.</span>
  </a>
  <a class="card" href="commands/stub/">
    <strong>Generate stubs</strong>
    <span>Create typed Python `.pyi` helpers from existing `.ftl` messages.</span>
  </a>
  <a class="card" href="commands/untranslated/">
    <strong>Find placeholders</strong>
    <span>Detect messages that still use their key as the translated value.</span>
  </a>
  <a class="card" href="configuration/">
    <strong>Use pyproject.toml</strong>
    <span>Keep long command defaults in command-specific config sections.</span>
  </a>
</div>

## Configured workflow

With configuration in `pyproject.toml`, the common workflow is concise:

```shell
ftl extract
ftl stub
ftl untranslated
```

Use `ftl --help` or a command-specific help page for the complete CLI surface.
