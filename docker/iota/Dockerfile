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

RUN cargo build --profile ${PROFILE} --bin iota --features ${CARGO_BUILD_FEATURES:=default}

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
mv $TARGET_DIR/iota ./;

# Production image
FROM debian:bookworm-slim AS runtime

ARG WORKDIR="/iota"
WORKDIR "$WORKDIR"

# Install runtime dependencies and tools
RUN apt update && apt install -y libpq5 ca-certificates curl

COPY --from=builder /iota/iota /usr/local/bin

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
