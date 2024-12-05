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
RUN apt update && apt install -y cmake clang lld git

# Configure Rust to use clang and lld as the linker
RUN mkdir -p ~/.cargo && \
    echo -e "[target.x86_64-unknown-linux-gnu]\nlinker = \"clang\"\nrustflags = [\"-C\", \"link-arg=-fuse-ld=lld\"]" > ~/.cargo/config.toml

# Install additional dependencies
RUN apt install -y libpq5 libpq-dev ca-certificates

# Clone the repository and checkout the devnet branch
RUN git clone https://github.com/iotaledger/iota .
RUN git checkout devnet

RUN cargo build --profile ${PROFILE} \
  --bin iota \
  --bin iota-rosetta \
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
mv $TARGET_DIR/iota ./ && \
mv $TARGET_DIR/iota-rosetta ./;

# Production image
FROM debian:bookworm-slim AS runtime

ARG WORKDIR="/iota"
WORKDIR "$WORKDIR"

# Install runtime dependencies and tools
RUN apt update && apt install -y libpq5 ca-certificates curl

# Install rosetta-cli
RUN curl -sSfL https://raw.githubusercontent.com/coinbase/rosetta-cli/master/scripts/install.sh | sh -s

# Download the genesis file and create the genesis state
RUN mkdir -p devnet && curl -fL -o devnet/genesis.blob https://github.com/iotaledger/iota-genesis/raw/main/devnet/genesis.blob

COPY --from=builder /iota/iota /usr/local/bin
COPY --from=builder /iota/iota-rosetta /usr/local/bin
COPY --from=builder /iota/crates/iota-config/data/fullnode-template.yaml /iota/devnet/fullnode.yaml
RUN /usr/local/bin/iota genesis

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
