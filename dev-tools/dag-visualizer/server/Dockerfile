ARG RUST_IMAGE_VERSION=trixie
FROM rust:${RUST_IMAGE_VERSION} AS builder
ARG PROFILE=release
ARG TARGET_FOLDER=target/release
WORKDIR /iota
RUN apt update && apt install -y --no-install-recommends cmake clang lld protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Configure clang + lld for faster linking (architecture-aware)
RUN mkdir -p ~/.cargo && \
    ARCH=$(dpkg --print-architecture) && \
    if [ "$ARCH" = "amd64" ]; then TARGET="x86_64-unknown-linux-gnu"; \
    elif [ "$ARCH" = "arm64" ]; then TARGET="aarch64-unknown-linux-gnu"; \
    else TARGET="x86_64-unknown-linux-gnu"; fi && \
    echo -e "[target.$TARGET]\nlinker = \"clang\"\nrustflags = [\"-C\", \"link-arg=-fuse-ld=lld\"]" > ~/.cargo/config.toml

# Copy root workspace (needed by path dependencies dag-visualizer-proto, typed-store)
COPY consensus consensus
COPY crates crates
COPY external-crates external-crates
COPY iota-execution iota-execution
COPY Cargo.toml Cargo.lock ./

# Copy the standalone server
COPY dev-tools/dag-visualizer/server dev-tools/dag-visualizer/server

# Build from the server's own workspace
WORKDIR /iota/dev-tools/dag-visualizer/server
RUN cargo build --profile ${PROFILE} --bin dag-visualizer-server && \
    cp $TARGET_FOLDER/dag-visualizer-server ./dag-visualizer-server

FROM debian:trixie-slim AS runtime
RUN apt update && apt install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /iota/dev-tools/dag-visualizer/server/dag-visualizer-server /usr/local/bin/
EXPOSE 9186
ENTRYPOINT ["dag-visualizer-server"]
