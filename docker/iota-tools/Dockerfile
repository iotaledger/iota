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
RUN apt update && apt install -y cmake clang lld libudev-dev protobuf-compiler jq

# Configure Rust to use clang and lld as the linker
RUN mkdir -p ~/.cargo && \
    echo -e "[target.x86_64-unknown-linux-gnu]\nlinker = \"clang\"\nrustflags = [\"-C\", \"link-arg=-fuse-ld=lld\"]" > ~/.cargo/config.toml

# Install additional dependencies (git is needed when fetching move dependencies during builds)
RUN apt install -y libpq5 libpq-dev ca-certificates git

# Copy in all crates, Cargo.toml, Cargo.lock, and binary build list
COPY crates crates
COPY docs docs
COPY examples examples
COPY external-crates external-crates
COPY iota-execution iota-execution
COPY Cargo.toml Cargo.lock binary-build-list.json ./

# Build binaries with their required features looked up from binary-build-list.json
RUN BINARIES="\
      iota-node \
      iota-analytics-indexer \
      iota \
      iota-localnet \
      iota-faucet \
      iota-cluster-test \
      iota-tool \
      iota-genesis-builder \
    "; \
    for binary in $BINARIES; do \
      features=$(jq -r --arg bin "$binary" '.release_binaries[$bin] // [] | join(",")' binary-build-list.json); \
      echo ">>> Building $binary with features: $features"; \
      if [ -z "$features" ]; then \
        cargo build --profile ${PROFILE} --bin "$binary"; \
      else \
        cargo build --profile ${PROFILE} --bin "$binary" --features "$features"; \
      fi; \
      cp $TARGET_FOLDER/$binary ./$binary; \
    done

# Production image
FROM debian:trixie-slim AS runtime

ARG WORKDIR="/iota"
WORKDIR "$WORKDIR"

# Install runtime dependencies and tools
RUN apt update && apt install -y libpq5 ca-certificates curl git

COPY --from=builder /iota/iota-node /usr/local/bin
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
