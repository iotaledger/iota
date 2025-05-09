This crate contains a Command Line Interface light client for IOTA.

# What is a light client?

A light client allows checking the authenticity and validity of on-chain state, such as transactions, their effects including events and object contents, without the cost of running a full node.

Running a _full node_ requires downloading the full sequence of all transaction and re-executing them. Then the full state of the blockchain is available locally to serve reads. This is however an expensive process in terms of network bandwidth needed to download the full sequence of transactions, as well as CPU to re-execute it, and storage to store the full state of the blockchain.

Alternatively, a _light client_ only needs to download minimal information to authenticate blockchain state. Specifically in IOTA, the light client needs to _sync_ all end-of-epoch checkpoints that contain information about the committee in the next epoch. Sync involves downloading the checkpoints and checking their validity by checking their certificate.

Once all end-of-epoch checkpoints are downloaded and checked, any event or current object can be checked for its validity. To do that the light client downloads the checkpoint in which the transaction was executed, and the effects structure that summarizes its effects on the system, including events emitted and objects created. The chain of validity from the checkpoint to the effects and its contents is checked via the certificate on the checkpoint and the hashes of all structures.

## Ensuring valid data display

A light client can ensure the correctness of the event and object data using the techniques defined above. However, the light client CLI utility also needs to pretty-print the structures in JSON, which requires knowledge of the correct type for each event or object. Types themselves are defined in modules that have been uploaded by past transactions. Therefore to ensure correct display the light client authenticates that all modules needed to display sought items are also correct.

# Usage

The light client requires a config file and a directory to cache checkpoints, and then can be used to check the validity of transaction and their events or of objects.

## Setup

The config file for the light client takes a URL for a full node, a directory to store checkpoint summaries (that must exist) and within the directory the name of the genesis blob for the IOTA network.

```
# A full node JSON RPC endpoint to query the latest network state (mandatory)
rpc_url: "https://api.mainnet.iota.cafe"

# A full node GraphQL RPC endpoint to query end-of-epoch checkpoints (optional if archive store config is provided)
graphql_url: "https://graphql.mainnet.iota.cafe"

# Local directory to store checkpoint summaries and other synchronization data (mandatory) 
checkpoints_dir: "checkpoints_mainnet"

# A URL to download or copy the genesis blob file from (optional if genesis blob is already present in checkpoints_dir)
genesis_blob_download_url: "https://dbfiles.mainnet.iota.cafe/genesis.blob"

# A flag to set whether the light client should always sync before checking an object or a transaction for inclusion (mandatory)
sync_before_check: true

# A config for an object store that gets populated by a historical checkpoint writer (optional)
checkpoint_store_config:
  object-store: "S3"
  aws-endpoint: "https://checkpoints.mainnet.iota.cafe"
  aws-virtual-hosted-style-request: true
  no-sign-request: true
  aws-region: "weur"
  object-store-connection-limit: 20

# A config for an object store that gets populated by an archiver (optional)
archive_store_config:
  object-store: "S3"
  aws-endpoint: "https://archive.mainnet.iota.cafe"
  aws-virtual-hosted-style-request: true
  no-sign-request: true
  aws-region: "weur"
  object-store-connection-limit: 20
```

## Sync

Every day there is a need to download new checkpoints through sync by doing:

```
$ iota-light-client --config testnet.yaml sync
```

Where `testnet.yaml` is the config file above.

This command will download all end-of-epoch checkpoints, and check them for validity. They will be cached within the checkpoint summary directory for use by future invocations.

Internally, sync works in two steps. It first downloads the end-of-epoch checkpoint numbers into the `checkpoints.yaml` file (which needs to be present in the checkpoint summaries directory). Next, it downloads the corresponding checkpoint summaries.

## Check Transaction

To check whether a transaction was executed in the testnet and its effects and events exist, run:

```
$ iota-light-client --config testnet.yaml check-transaction <transaction-digest>
```

where `transaction-digest` is a base58 encoded string. If the transaction has been executed in the past, its digest, the effects, and all events are displayed. Events are printed in JSON. Otherwise an error is shown.

If you set `sync_before_check: true` in the config, the light client will first sync itself to the latest network state before checking the transaction.

## Check Object

To check whether an object exists in the testnet, run:

```
$ iota-light-client --config testnet.yaml check-object <object-id>
```

where `object-id` is a hex encoded string with a 0x prefix. If the object exists, it is printed in JSON. Otherwise an error is shown.

If you set `sync_before_check: true` in the config, the light client will first sync itself to the latest network state before checking the object.
