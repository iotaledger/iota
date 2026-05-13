# Build image (the specific rust version can also be passed, e.g. "1.91-trixie")
ARG RUST_IMAGE_VERSION=trixie
FROM rust:${RUST_IMAGE_VERSION} AS builder

ARG PROFILE=release
ARG TARGET_FOLDER=target/release
ARG CARGO_BUILD_FEATURES
# The GIT_REVISION environment variable is used during build time inside the rust crates
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION

WORKDIR "/iota"

# Install build dependencies, including clang and lld for faster linking
RUN apt update && apt install -y cmake clang lld protobuf-compiler

# Configure Rust to use clang and lld as the linker
RUN mkdir -p ~/.cargo && \
    echo -e "[target.x86_64-unknown-linux-gnu]\nlinker = \"clang\"\nrustflags = [\"-C\", \"link-arg=-fuse-ld=lld\"]" > ~/.cargo/config.toml

# Install additional dependencies
RUN apt install -y ca-certificates libpq5 libpq-dev

# Copy in all crates, Cargo.toml, and Cargo.lock
COPY crates crates
COPY external-crates external-crates
COPY iota-execution iota-execution
COPY Cargo.toml Cargo.lock ./

# Build the binaries and copy them to the working directory
RUN cargo build --profile ${PROFILE} \
      --bin iota-localnet \
      --bin iota-tool \
      --features ${CARGO_BUILD_FEATURES:=default} && \
    cp $TARGET_FOLDER/iota-localnet ./iota-localnet && \
    cp $TARGET_FOLDER/iota-tool ./iota-tool

# Production image
FROM debian:trixie-slim AS runtime

ARG WORKDIR="/iota"
WORKDIR "$WORKDIR"

# Install runtime dependencies and tools
RUN apt update && apt install -y ca-certificates curl libpq5
COPY --from=builder /iota/iota-localnet /usr/local/bin
COPY --from=builder /iota/iota-tool /usr/local/bin

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
