# syntax=docker/dockerfile:1
# Build image (the specific rust version can also be passed, e.g. "1.82-bookworm")
ARG RUST_IMAGE_VERSION=bookworm
FROM rust:${RUST_IMAGE_VERSION} AS builder

ARG PROFILE=release
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
RUN apt install -y libpq5 libpq-dev ca-certificates

# Copy in all crates, Cargo.toml, and Cargo.lock
COPY consensus consensus
COPY crates crates
COPY docs docs
COPY external-crates external-crates
COPY iota-execution iota-execution
COPY Cargo.toml Cargo.lock ./

# Build with cache mounts to speed up compilation
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/iota/target,sharing=locked \
    cargo build --profile ${PROFILE} \
      --bin iota-rosetta \
      --features ${CARGO_BUILD_FEATURES:=default} && \
    if [ -d target/release ]; then \
      cp target/release/iota-rosetta ./iota-rosetta; \
    elif [ -d target/debug ]; then \
      cp target/debug/iota-rosetta ./iota-rosetta; \
    else \
      echo "Error: No build directory found"; \
      exit 1; \
    fi

# Production image
FROM debian:bookworm-slim AS runtime

ARG WORKDIR="/iota"
WORKDIR "$WORKDIR"

# Install runtime dependencies and tools
RUN apt update && apt install -y libpq5 ca-certificates curl

# Install rosetta-cli (workaround because the install script is broken after rename)
RUN curl -sSfL https://raw.githubusercontent.com/coinbase/mesh-cli/master/scripts/install.sh | sed -e 's/^REPO=.*/REPO=mesh-cli/' | sh -s
COPY --from=builder /iota/iota-rosetta /usr/local/bin

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
