# `@iota/graphql-transport`

`@iota/graphql-transport` is part of the **IOTA Rebased SDK**, designed specifically for interacting with the IOTA Rebased protocol.

> **Note**: This package is currently supported **only in Testnet and Devnet**, it is **not yet supported in Mainnet**.

This package provides a `IotaTransport` that enables `IotaClient` to make requests using the RPC 2.0
(GraphQL) API instead of the JSON RPC API.

## Install

```bash
npm install --save @iota/graphql-transport
```

## Setup

```ts
import { IotaClientGraphQLTransport } from '@iota/graphql-transport';
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';

const client = new IotaClient({
    transport: new IotaClientGraphQLTransport({
        url: 'https://iota-testnet.iota.org/graphql',
        // When specified, the transport will fallback to JSON RPC for unsupported method and parameters
        fallbackFullNodeUrl: getFullnodeUrl('testnet'),
    }),
});
```

## Limitations

### Unsupported methods

The following methods are currently unsupported in IotaClientGraphQLTransport, and will either
error, or fallback to the JSON RPC API if a `fallbackFullNodeUrl` is provided:

- `subscribeTransaction`
- `subscribeEvents`
- `call`
- `getNetworkMetrics`
- `getMoveCallMetrics`
- `getAddressMetrics`
- `getEpochs`
- `dryRunTransactionBlock`
- `devInspectTransactionBlock`
- `executeTransactionBlock`
- `getParticipationMetrics`

### Unsupported parameters

Some supported methods in `IotaClientGraphQLTransport` do not support the full set of parameters
available in the JSON RPC API.

If an unsupported parameter is used, the request will error, or fallback to JSON RPC API if a
`fallbackFullNodeUrl` is provided.

- `getOwnedObjects`:
  - missing the `MatchAll`, `MatchAny`, `MatchNone`, and `Version` filters
- `queryEvents`:
  - missing the `MoveEventField`, `Module`, `TimeRange`, `All`, `Any`, `And`, and `Or` filters

### Unsupported fields

- `queryTransactionBlocks`, `getTransactionBlock`, and `multiGetTransactionBlocks`
  - missing `messageVersion`, `eventsDigest`, `sharedObjects`, `unwrapped`, `wrapped`, and
    `unwrappedThenDeleted` in effects
  - missing `id` for `events`
- `getStakes` and `getStakesByIds`
  - missing `validatorAddress`
- `getLatestIotaSystemState`
  - missing `stakingPoolMappingsId`, `inactivePoolsId`, `pendingActiveValidatorsId`,
    `validatorCandidatesId`
  - missing `reportRecords` on validators
- `getCurrentEpoch`
  - missing `reportRecords` on validators
- `queryEvents`
  - missing `id` for `events`
- `getCheckpoint` and `getCheckpoints`
  - missing `checkpointCommitments`
- `getCurrentEpoch`
  - missing `epochTotalTransactions`
- `getDynamicFields`
  - missing `objectId`, `digest` and `version` available for `DynamicObject` but not
    `DynamicField`

### Performance

Some may require multiple requests to properly resolve:

- `getDynamicFieldObject` requires 2 requests
- `queryTransactionBlocks`, `getTransactionBlock`, and `multiGetTransactionBlocks`
  - may require additional requests to load all `objectChanges`, `balanceChanges`,
    `dependencies` and `events`
- `getNormalizedMoveModule` and `getNormalizedMoveModulesByPage`
  - may require additional requests to load all `friends`, `functions`, and `structs`
- `getCheckpoint` and `getCheckpoints`,
  - may require additional requests to load all `transactionBlocks` and `validators`
- `getLatestIotaSystemState`, `getCurrentEpoch`, `getValidatorsApy` and `getCommitteeInfo`:
  - may require additional requests to load all `validators`

### Pagination

Page sizes and limits for paginated methods are based on the defaults and limits of the GraphQL API,
so page sizes and limits may be different than those returned by the JSON RPC API
