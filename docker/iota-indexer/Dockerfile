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

# iota-indexer needs postgres libpq5 and ca-certificates
RUN apt update && apt install -y libpq5 ca-certificates libpq-dev postgresql

COPY Cargo.toml Cargo.lock ./
COPY consensus consensus
COPY crates crates
COPY iota-execution iota-execution
COPY external-crates external-crates
COPY docs docs

RUN cargo build --profile ${PROFILE} --bin iota-indexer

# Copy the built binary to the working directory depending on the profile, so we can copy it to the runtime image
RUN mv target/$(if [ $PROFILE = "dev" ]; then echo "debug"; else echo "release";fi)/iota-indexer ./

# Production Image
FROM debian:bullseye-slim AS runtime

# Use jemalloc as memory allocator
RUN apt-get update && apt-get install -y libjemalloc-dev ca-certificates curl
ENV LD_PRELOAD /usr/lib/x86_64-linux-gnu/libjemalloc.so

WORKDIR "$WORKDIR/iota"
COPY --from=builder /iota/iota-indexer /usr/local/bin
RUN apt update && apt install -y libpq5 ca-certificates libpq-dev postgresql

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
