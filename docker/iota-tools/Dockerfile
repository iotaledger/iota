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
# libudev for Ledger hardware wallets
RUN apt update && apt install -y cmake clang lld libudev-dev protobuf-compiler

# Configure Rust to use clang and lld as the linker
RUN mkdir -p ~/.cargo && \
    echo -e "[target.x86_64-unknown-linux-gnu]\nlinker = \"clang\"\nrustflags = [\"-C\", \"link-arg=-fuse-ld=lld\"]" > ~/.cargo/config.toml

# Install additional dependencies (git is needed when fetching move dependencies during builds)
RUN apt install -y libpq5 libpq-dev ca-certificates git

# Copy in all crates, Cargo.toml, and Cargo.lock
COPY consensus consensus
COPY crates crates
COPY docs docs
COPY examples examples
COPY external-crates external-crates
COPY iota-execution iota-execution
COPY Cargo.toml Cargo.lock ./

# Build with cache mounts to speed up compilation
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/iota/target,sharing=locked \
    cargo build --profile ${PROFILE} \
      --bin iota-node \
      --bin stress \
      --bin iota-analytics-indexer \
      --bin iota \
      --bin iota-faucet \
      --bin iota-cluster-test \
      --bin iota-tool \
      --bin iota-genesis-builder \
      --features ${CARGO_BUILD_FEATURES:=default} && \
    if [ -d target/release ]; then \
      cp target/release/iota-node ./iota-node && \
      cp target/release/stress ./stress && \
      cp target/release/iota-analytics-indexer ./iota-analytics-indexer && \
      cp target/release/iota ./iota && \
      cp target/release/iota-faucet ./iota-faucet && \
      cp target/release/iota-cluster-test ./iota-cluster-test && \
      cp target/release/iota-tool ./iota-tool && \
      cp target/release/iota-genesis-builder ./iota-genesis-builder; \
    elif [ -d target/debug ]; then \
      cp target/debug/iota-node ./iota-node && \
      cp target/debug/stress ./stress && \
      cp target/debug/iota-analytics-indexer ./iota-analytics-indexer && \
      cp target/debug/iota ./iota && \
      cp target/debug/iota-faucet ./iota-faucet && \
      cp target/debug/iota-cluster-test ./iota-cluster-test && \
      cp target/debug/iota-tool ./iota-tool && \
      cp target/debug/iota-genesis-builder ./iota-genesis-builder; \
    else \
      echo "Error: No build directory found"; \
      exit 1; \
    fi

# Production image
FROM debian:bookworm-slim AS runtime

ARG WORKDIR="/iota"
WORKDIR "$WORKDIR"

# Install runtime dependencies and tools
RUN apt update && apt install -y libpq5 ca-certificates curl git

COPY --from=builder /iota/iota-node /usr/local/bin
COPY --from=builder /iota/stress /usr/local/bin
COPY --from=builder /iota/iota-analytics-indexer /usr/local/bin
COPY --from=builder /iota/iota /usr/local/bin
COPY --from=builder /iota/iota-faucet /usr/local/bin
COPY --from=builder /iota/iota-cluster-test /usr/local/bin
COPY --from=builder /iota/iota-tool /usr/local/bin
COPY --from=builder /iota/iota-genesis-builder /usr/local/bin

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
