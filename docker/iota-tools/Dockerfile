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
RUN apt update && apt install -y cmake clang lld

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

RUN cargo build --profile ${PROFILE} \
  --bin iota-node \
  --bin stress \
  --bin iota-bridge \
  --bin bridge-indexer \
  --bin iota-bridge-cli \
  --bin iota-analytics-indexer \
  --bin iota \
  --bin iota-faucet \
  --bin iota-cluster-test \
  --bin iota-tool \
  --features ${CARGO_BUILD_FEATURES:=default}

# Copy the built binary to the working directory depending on the output folder of the profile,
# so we can copy it to the runtime image
RUN if [ -d target/release ]; then \
  TARGET_DIR="target/release"; \
elif [ -d target/debug ]; then \
  TARGET_DIR="target/debug"; \
else \
  echo "Error: No build directory found"; \
  exit 1; \
fi && \
mv $TARGET_DIR/iota-node ./ && \
mv $TARGET_DIR/stress ./ && \
mv $TARGET_DIR/iota-bridge ./ && \
mv $TARGET_DIR/bridge-indexer ./ && \
mv $TARGET_DIR/iota-bridge-cli ./ && \
mv $TARGET_DIR/iota-analytics-indexer ./ && \
mv $TARGET_DIR/iota ./ && \
mv $TARGET_DIR/iota-faucet ./ && \
mv $TARGET_DIR/iota-cluster-test ./ && \
mv $TARGET_DIR/iota-tool ./;

# Production image
FROM debian:bookworm-slim AS runtime

ARG WORKDIR="/iota"
WORKDIR "$WORKDIR"

# Install runtime dependencies and tools
RUN apt update && apt install -y libpq5 ca-certificates curl

COPY --from=builder /iota/iota-node /usr/local/bin
COPY --from=builder /iota/stress /usr/local/bin
COPY --from=builder /iota/iota-bridge /usr/local/bin
COPY --from=builder /iota/bridge-indexer /usr/local/bin
COPY --from=builder /iota/iota-bridge-cli /usr/local/bin
COPY --from=builder /iota/iota-analytics-indexer /usr/local/bin
COPY --from=builder /iota/iota /usr/local/bin
COPY --from=builder /iota/iota-faucet /usr/local/bin
COPY --from=builder /iota/iota-cluster-test /usr/local/bin
COPY --from=builder /iota/iota-tool /usr/local/bin

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
