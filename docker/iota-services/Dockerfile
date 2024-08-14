FROM rust:1.75-bullseye  AS chef
WORKDIR iota
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION
RUN apt-get update && apt-get install -y cmake clang libpq5 ca-certificates libpq-dev postgresql

# Build application
FROM chef AS builder 

COPY Cargo.toml Cargo.lock ./
COPY consensus consensus
COPY crates crates
COPY iota-execution iota-execution
COPY narwhal narwhal
COPY external-crates external-crates
RUN mkdir /iota/bin/
RUN find /iota/target/release/ -maxdepth 1 -type f -executable -print 
RUN find /iota/target/release/ -maxdepth 1 -type f -executable -print | xargs cp -t /iota/bin/

# Production Image
FROM debian:bullseye-slim AS runtime
WORKDIR iota
COPY --from=builder /iota/bin/* /usr/local/bin
RUN apt update && apt install -y libpq5 libpq-dev postgresql

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION

