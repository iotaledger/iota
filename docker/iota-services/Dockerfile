FROM rust:bullseye AS chef
WORKDIR iota
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION

RUN apt-get update && apt-get install -y cmake clang libpq5 ca-certificates libpq-dev postgresql

# Build application
FROM chef AS builder 

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

