FROM ubuntu:latest AS chef
WORKDIR iota
ARG GIT_REVISION
ENV GIT_REVISION=$GIT_REVISION
RUN apt-get update && apt-get install -y build-essential libssl-dev pkg-config curl cmake clang ca-certificates
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

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

RUN curl -sSfL https://raw.githubusercontent.com/coinbase/rosetta-cli/master/scripts/install.sh | sh -s

COPY Cargo.toml Cargo.lock ./
COPY consensus consensus
COPY crates crates
COPY iota-execution iota-execution
COPY narwhal narwhal
COPY external-crates external-crates
COPY docs docs

# TODO: Remove "--mount=type=ssh" when iota-sim is public https://github.com/iotaledger/iota/issues/2149
RUN --mount=type=ssh cargo build --release --bin iota --bin iota-rosetta

# Production Image
FROM ubuntu:latest AS runtime
WORKDIR iota
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /iota/target/release/iota /usr/local/bin
COPY --from=builder /iota/target/release/iota-rosetta /usr/local/bin
COPY --from=builder /iota/bin/rosetta-cli /usr/local/bin
COPY --from=builder /iota/crates/iota-config/data/fullnode-template.yaml /iota/devnet/fullnode.yaml
RUN /usr/local/bin/iota genesis

ARG BUILD_DATE
LABEL build-date=$BUILD_DATE
LABEL git-revision=$GIT_REVISION
