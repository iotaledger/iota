# Build application
#
# Copy in all crates, Cargo.toml and Cargo.lock unmodified,
# and build the application.
FROM rust:bullseye AS builder
ARG PROFILE=release
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION
WORKDIR "$WORKDIR/iota"

RUN apt-get update && apt-get install -y cmake clang

COPY Cargo.toml Cargo.lock ./
COPY consensus consensus
COPY crates crates
COPY iota-execution iota-execution
COPY external-crates external-crates
COPY docs docs

RUN cargo build --profile ${PROFILE} --bin iota-node

# Copy the built binary to the working directory depending on the profile, so we can copy it to the runtime image
RUN mv target/$(if [ $PROFILE = "dev" ]; then echo "debug"; else echo "release";fi)/iota-node ./

# Production Image
FROM debian:bullseye-slim AS runtime

# Use jemalloc as memory allocator
RUN apt-get update && apt-get install -y libjemalloc-dev ca-certificates curl
ENV LD_PRELOAD /usr/lib/x86_64-linux-gnu/libjemalloc.so

WORKDIR "$WORKDIR/iota"

# Both bench and release profiles copy from release dir
COPY --from=builder /iota/iota-node /opt/iota/bin/iota-node

# Support legacy usages of /usr/local/bin/iota-node
COPY --from=builder /iota/iota-node /usr/local/bin

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
