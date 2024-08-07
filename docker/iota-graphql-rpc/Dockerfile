# Build application
#
# Copy in all crates, Cargo.toml and Cargo.lock unmodified,
# and build the application.
FROM rust:bullseye AS builder
ARG PROFILE=release
ENV OUT_DIR=$PROFILE
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION
WORKDIR "$WORKDIR/iota"
RUN apt-get update && apt-get install -y cmake clang libpq-dev

COPY Cargo.toml Cargo.lock ./
COPY consensus consensus
COPY crates crates
COPY iota-execution iota-execution
COPY narwhal narwhal
COPY external-crates external-crates

RUN cargo build --profile ${PROFILE} --bin iota-graphql-rpc
RUN mv target/$(if [ $PROFILE = "dev" ]; then echo "debug"; else echo "release";fi)/iota-graphql-rpc ./


# Production Image
FROM debian:bullseye-slim AS runtime
WORKDIR "$WORKDIR/iota"
# Both bench and release profiles copy from release dir
RUN mkdir -p /opt/iota/bin
COPY --from=builder /iota/iota-graphql-rpc /opt/iota/bin
RUN apt-get update && apt-get install -y libpq5 ca-certificates libpq-dev postgresql

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
