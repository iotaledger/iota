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

COPY .git/ .git/
COPY consensus consensus
COPY crates crates
COPY docs docs
COPY examples examples
COPY external-crates external-crates
COPY iota-execution iota-execution
COPY Cargo.toml Cargo.lock ./

RUN cargo build --profile ${PROFILE} --bin iota-proxy

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
mv $TARGET_DIR/iota-proxy ./;


# Production image
FROM debian:bookworm-slim AS runtime

ARG WORKDIR="/iota"
WORKDIR "$WORKDIR"

# Install runtime dependencies and tools
RUN apt update && apt install -y libpq5 ca-certificates curl

# Install jemalloc as the default allocator used for memory profiling on supported
# architectures and create a symlink for the correct version based on the architecture
RUN ARCH=$(dpkg --print-architecture) && \
  if [ "$ARCH" = "amd64" ] || [ "$ARCH" = "arm64" ]; then \
    apt update && apt install -y libjemalloc-dev; \
    ln -sf $(ldconfig -p | grep jemalloc | awk '{print $4}' | sort -u | head -n 1) /usr/lib/libjemalloc.so; \
  else \
    echo "Unsupported architecture: $ARCH. Only amd64 and arm64 are supported."; \
  exit 1; \
fi

# Set LD_PRELOAD to the symlinked path
ENV LD_PRELOAD=/usr/lib/libjemalloc.so

COPY --from=builder /iota/iota-proxy /usr/local/bin

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
