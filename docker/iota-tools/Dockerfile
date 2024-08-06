# Build application
#
# Copy in all crates, Cargo.toml and Cargo.lock unmodified,
# and build the application.
FROM rust:bullseye AS base
ARG PROFILE=release
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION
WORKDIR "$WORKDIR/iota"
RUN apt-get update && apt-get install -y cmake clang libpq5 libpq-dev

COPY Cargo.toml Cargo.lock ./
COPY consensus consensus
COPY crates crates
COPY iota-execution iota-execution
COPY narwhal narwhal
COPY external-crates external-crates

FROM base AS builder-light

RUN cargo build --profile ${PROFILE} --bin iota-test-validator

FROM builder-light AS builder

RUN cargo build --profile ${PROFILE} \
    --bin iota-node \
    --bin iota \
    --bin iota-faucet \
    --bin stress \
    --bin iota-cluster-test \
    --bin iota-tool

# Light runtime for iota-test-validator
FROM debian:bullseye-slim AS runtime-light
WORKDIR "$WORKDIR/iota"

# iota-tool needs libpq at runtime
RUN apt-get update && apt-get install -y libpq5 libpq-dev

COPY --from=builder-light /iota/target/release/iota-test-validator /usr/local/bin

# Production Image
FROM debian:bullseye-slim AS runtime
WORKDIR "$WORKDIR/iota"

# iota-tool needs libpq at runtime
RUN apt-get update && apt-get install -y libpq5 libpq-dev

COPY --from=builder /iota/target/release/iota-node /usr/local/bin
COPY --from=builder /iota/target/release/iota /usr/local/bin
COPY --from=builder /iota/target/release/iota-faucet /usr/local/bin
COPY --from=builder /iota/target/release/stress /usr/local/bin
COPY --from=builder /iota/target/release/iota-cluster-test /usr/local/bin
COPY --from=builder /iota/target/release/iota-tool /usr/local/bin
COPY --from=builder /iota/target/release/iota-test-validator /usr/local/bin

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
