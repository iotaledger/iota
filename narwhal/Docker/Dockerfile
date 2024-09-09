# Build application
#
# Copy in all crates, Cargo.toml and Cargo.lock unmodified,
# and build the application.
FROM rust:1.74-bullseye AS builder
ARG PROFILE=release
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION
WORKDIR "$WORKDIR/iota"
RUN apt-get update && apt-get install -y cmake clang

# TODO: Remove when iota-sim is public https://github.com/iotaledger/iota/issues/2149
RUN mkdir -p -m 0700 ~/.ssh && ssh-keyscan github.com >> ~/.ssh/known_hosts
RUN --mount=type=ssh <<EOT
  set -e
  echo "Setting Git SSH protocol"
  (
    set +e
    ssh -T git@github.com
    if [ ! "$?" = "1" ]; then
      echo "No GitHub SSH key loaded, exiting..."
      exit 1
    fi
  )
EOT
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

COPY Cargo.toml Cargo.lock ./
COPY consensus consensus
COPY crates crates
COPY iota-execution iota-execution
COPY narwhal narwhal
COPY external-crates external-crates
COPY docs docs

RUN --mount=type=ssh cargo build --profile ${PROFILE} --bin narwhal-node
RUN --mount=type=ssh cargo build --profile ${PROFILE} --features=benchmark --bin narwhal-benchmark-client

# Production Image
FROM debian:bullseye-slim AS runtime
# Use jemalloc as memory allocator
RUN apt-get update && apt-get install -y libjemalloc-dev
ENV LD_PRELOAD /usr/lib/x86_64-linux-gnu/libjemalloc.so
ARG PROFILE=release
WORKDIR "$WORKDIR/iota"

# Copy the Narwhal node binary to bin folder
COPY --from=builder iota/target/${PROFILE}/narwhal-node bin/

# This is used for testing a cluster by generating load.
COPY --from=builder iota/target/${PROFILE}/narwhal-benchmark-client bin/

# Copy the entry point file
COPY narwhal/Docker/entry.sh ./

# Now add the entry point
CMD ./entry.sh

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
