# IOTA Network Docker Compose

This was tested using MacOS 14.3.1, Docker Compose: v2.13.0.

This compose brings up 3 validators and 1 fullnode.

Steps for running:

1. run compose

```
(optional) `rm -r /tmp/iota`
docker compose up
```

For load generation against this network, use the `stress` tool from the [`iotaledger/network-benchmark`](https://github.com/iotaledger/network-benchmark) repo.

**additional info**
The version of `iota` which is used to generate the genesis outputs must be on the same protocol version as the fullnode/validators (eg: `iotaledger/iota-node:mainnet-v1.19.1`)
Here's an example of how to build a `iota` binary that creates a genesis which is compatible with the release: `v1.19.1`

```
git checkout releases/iota-v1.19.0-release
cargo build --bin iota
```

you can also use `iota-network/Dockerfile` for building genesis
