# Build image for Anthropic's OSS Scanner: https://github.com/anthropics/oss-scanner
#
# The scanner builds this file from the repository root with network access, then analyses the
# image with no network at all. Everything the build and the tests need is therefore fetched
# here, and cargo is switched to offline mode for the later `cargo` runs.
#
# Local check: docker build -f .oss-scanner/Dockerfile -t ftl-extract-scan .
#              docker run --rm --network none ftl-extract-scan cargo test --workspace
FROM rust:1-bookworm

# Root ignores file permissions, and a test relies on a read-only directory refusing writes.
RUN useradd --create-home scan && mkdir /src && chown scan:scan /src
USER scan
ENV CARGO_HOME=/home/scan/.cargo

WORKDIR /src
COPY --chown=scan:scan . .

# All crates in Cargo.lock, for every platform and every target (tests, benches included).
RUN cargo fetch --locked

ENV CARGO_NET_OFFLINE=true \
    CARGO_TERM_COLOR=never \
    PATH=/src/target/debug:$PATH

# Builds the `ftl` binary into /src/target/debug and every test and bench target.
RUN cargo build --workspace --all-targets --locked

# A failing test must not fail the image: the scan is still useful without a green suite.
RUN cargo test --workspace --locked \
    || echo "WARNING: cargo test failed; the image is still built for the scan."
