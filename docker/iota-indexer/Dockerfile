# Build application
#
# Copy in all crates, Cargo.toml and Cargo.lock unmodified,
# and build the application.
FROM rust:bullseye AS builder
ARG PROFILE=release
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION
WORKDIR "$WORKDIR/iota"

# iota-indexer needs postgres libpq5 and ca-certificates
RUN apt update && apt install -y libpq5 ca-certificates libpq-dev postgresql

RUN apt-get update && apt-get install -y cmake clang

COPY Cargo.toml Cargo.lock ./
COPY consensus consensus
COPY crates crates
COPY iota-execution iota-execution
COPY narwhal narwhal
COPY external-crates external-crates

RUN cargo build --profile ${PROFILE} --bin iota-indexer
RUN mv target/$(if [ $PROFILE = "dev" ]; then echo "debug"; else echo "release";fi)/iota-graphql-rpc ./

# Production Image
FROM debian:bullseye-slim AS runtime
WORKDIR "$WORKDIR/iota"
COPY --from=builder /iota/iota-indexer /usr/local/bin
RUN apt update && apt install -y libpq5 ca-certificates libpq-dev postgresql

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
