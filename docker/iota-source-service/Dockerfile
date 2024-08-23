FROM rust:1.75-bullseye  AS chef
WORKDIR iota
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION
RUN apt-get update && apt-get install -y cmake clang

FROM chef AS builder 

# Build application
COPY Cargo.toml Cargo.lock ./
COPY consensus consensus
COPY crates crates
COPY iota-execution iota-execution
COPY narwhal narwhal
COPY external-crates external-crates
COPY docs docs
RUN cargo build --release \
    --bin iota-source-validation-service

# Production Image
FROM debian:bullseye-slim AS runtime
WORKDIR iota
RUN apt update && apt install -y git
COPY --from=builder /iota/target/release/iota-source-validation-service /usr/local/bin
COPY crates/iota-source-validation-service/config.toml /var/iota/

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
