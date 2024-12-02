# Build application
#
# Copy in all crates, Cargo.toml and Cargo.lock unmodified,
# and build the application.
FROM rust:bullseye AS builder
ARG PROFILE=release
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION
ARG CARGO_BUILD_FEATURES
WORKDIR "/iota"

RUN apt-get update && apt-get install -y cmake clang libpq5 libpq-dev

COPY Cargo.toml Cargo.lock ./
COPY consensus consensus
COPY crates crates
COPY iota-execution iota-execution
COPY external-crates external-crates
COPY docs docs

RUN cargo build --profile ${PROFILE} --bin iota --features ${CARGO_BUILD_FEATURES:=default}
RUN mv target/$(if [ $PROFILE = "dev" ]; then echo "debug"; else echo "release";fi)/iota ./

# Production Image
FROM debian:bullseye-slim AS runtime

# iota-tool needs libpq at runtime
RUN apt-get update && apt-get install -y libpq5 libpq-dev curl

COPY --from=builder /iota/iota /usr/local/bin

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
