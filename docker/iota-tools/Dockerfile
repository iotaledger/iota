# Build application
#
# Copy in all crates, Cargo.toml and Cargo.lock unmodified,
# and build the application.
FROM rust:bullseye AS builder
ARG PROFILE=release
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION
WORKDIR "$WORKDIR/iota"
RUN apt-get update && apt-get install -y cmake clang libpq5 libpq-dev

# TODO: Remove when iota-sim is public
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

RUN --mount=type=ssh cargo build --profile ${PROFILE} \
    --bin iota-node \
    --bin iota \
    --bin iota-faucet \
    --bin stress \
    --bin iota-cluster-test \
    --bin iota-tool

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

ARG BUILD_DATE
ARG GIT_REVISION
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
