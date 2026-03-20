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

# Build the binaries and copy them to the working directory
RUN cargo build --profile ${PROFILE} \
      --bin iota-node \
      --bin stress \
      --bin iota-analytics-indexer \
      --bin iota \
      --bin iota-localnet \
      --bin iota-faucet \
      --bin iota-cluster-test \
      --bin iota-tool \
      --bin iota-genesis-builder \
      --features ${CARGO_BUILD_FEATURES:=default} && \
    cp $TARGET_FOLDER/iota-node ./iota-node && \
    cp $TARGET_FOLDER/stress ./stress && \
    cp $TARGET_FOLDER/iota-analytics-indexer ./iota-analytics-indexer && \
    cp $TARGET_FOLDER/iota ./iota && \
    cp $TARGET_FOLDER/iota-localnet ./iota-localnet && \
    cp $TARGET_FOLDER/iota-faucet ./iota-faucet && \
    cp $TARGET_FOLDER/iota-cluster-test ./iota-cluster-test && \
    cp $TARGET_FOLDER/iota-tool ./iota-tool && \
    cp $TARGET_FOLDER/iota-genesis-builder ./iota-genesis-builder

# Production image
FROM debian:trixie-slim AS runtime

ARG WORKDIR="/iota"
WORKDIR "$WORKDIR"

# Install runtime dependencies and tools
RUN apt update && apt install -y libpq5 ca-certificates curl git

COPY --from=builder /iota/iota-node /usr/local/bin
COPY --from=builder /iota/stress /usr/local/bin
COPY --from=builder /iota/iota-analytics-indexer /usr/local/bin
COPY --from=builder /iota/iota /usr/local/bin
COPY --from=builder /iota/iota-localnet /usr/local/bin
COPY --from=builder /iota/iota-faucet /usr/local/bin
COPY --from=builder /iota/iota-cluster-test /usr/local/bin
COPY --from=builder /iota/iota-tool /usr/local/bin
COPY --from=builder /iota/iota-genesis-builder /usr/local/bin

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
