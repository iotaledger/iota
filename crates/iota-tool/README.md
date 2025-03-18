# iota-tool

`iota-tool` contains assorted debugging utilities for IOTA.

## Install `iota-tool`

### HomeBrew

You can use [Homebrew](https://brew.sh/) on macOS, Linux, or Windows Subsystem for Linux to install `iota` and [`iota-tool`](https://github.com/iotaledger/iota/tree/develop/crates/iota-tool):

```sh
brew install iotaledger/tap/iota
```

### Build From Source

You can build and run `iota-tool` from source with:

```sh
cargo run --bin iota-tool -- <args>
```

## Commands

```shell
Usage: iota-tool <COMMAND>

Commands:
  locked-object                  Inspect if a specific object is or all gas objects owned by an address are locked by
                                 validators
  fetch-object                   Fetch the same object from all validators
  fetch-transaction              Fetch the effects association with transaction `digest`
  db-tool                        Tool to read validator & node db
  verify-archive                 Tool to verify the archive store
  print-archive-manifest         Tool to print the archive manifest
  update-archive-manifest        Tool to update the archive manifest
  verify-archive-from-checksums  Tool to verify the archive store by comparing file checksums
  dump-archive                   Tool to print archive contents in checkpoint range
  dump-packages                  Download all packages to the local filesystem from a GraphQL service. Each package gets its
                                 own sub-directory, named for its ID on chain and version containing two metadata files
                                 (linkage.json and origins.json), a file containing the overall object and a file for every
                                 module it contains. Each module file is named for its module name, with a .mv suffix, and
                                 contains Move bytecode (suitable for passing into a disassembler)
  dump-validators                
  dump-genesis                   
  fetch-checkpoint               Fetch authenticated checkpoint information at a specific sequence number. If sequence
                                 number is not specified, get the latest authenticated checkpoint
  anemo                          Network tools for interacting with Anemo servers
  restore-db                     
  download-db-snapshot           Downloads the legacy database snapshot via cloud object store, outputs to local disk
  download-formal-snapshot       Downloads formal database snapshot via cloud object store, outputs to local disk
  replay                         
  sign-transaction               Ask all validators to sign a transaction through AuthorityAggregator
  help                           Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `anemo` tools

You can use the anemo CLI tools to ping or call an RPC on an Anemo server. Note that (for now) this uses randomly generated keys, so a server or method that restricts access to allowlisted peers will reject connections from this tool.

Anemo networks are identified by a "server name" that the client must match. Server names you may want to use:

- IOTA discovery and state sync: `iota`

### ping

Example command to ping an anemo server:

```sh
SERVER_NAME="iota"; \
ADDRESS="1.2.3.4:5678"; \
iota-tool anemo ping --server-name "$SERVER_NAME" "$ADDRESS"
```

### call

`iota-tool` has been preconfigured to support RPC calls using [RON (Rusty Object Notation)](https://crates.io/crates/ron) for the following servivces:

- IOTA: `Discovery` and `StateSync`

Example command to send an RPC:

```sh
SERVER_NAME="iota"; \
ADDRESS="1.2.3.4:5678"; \
SERVICE_NAME="StateSync"; \
METHOD_NAME="GetCheckpointSummary"; \
REQUEST="BySequenceNumber(123)"; \
iota-tool \
    anemo call --server-name "$SERVER_NAME" "$ADDRESS" "$SERVICE_NAME" "$METHOD_NAME" "$REQUEST"
```
