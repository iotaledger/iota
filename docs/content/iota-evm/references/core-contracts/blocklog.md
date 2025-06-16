---
description: The `blocklog` contract keeps track of the blocks of requests processed by the chain.
image: /img/logo/WASP_logo_dark.png
tags:
  - core-contract
  - core-contract-blocklog
  - reference
---

# The `blocklog` Contract

The `blocklog` contract is one of the [core contracts](overview.md) on each IOTA Smart Contracts chain.

The `blocklog` contract keeps track of the blocks of requests processed by the chain, providing views to get request
status, receipts, block, and event details.

To avoid having a monotonically increasing state size, only the latest `N`
blocks (and their events and receipts) are stored. This parameter can be configured
when deploying the chain.

## Views

### `getBlockInfo`

Returns information about the block with index `blockIndex`.

#### Parameters

| Name       | Type | Optional | Description                                |
| ---------- | ---- | -------- | ------------------------------------------ |
| blockIndex | u32  | Yes      | The block index. Default: the latest block |

#### Returns

| Name       | Type                               | Description                     |
| ---------- | ---------------------------------- | ------------------------------- |
| blockIndex | u32                                | The block Index                 |
| blockInfo  | *[BlockInfo](./types.md#blockinfo) | The information about the block |

### `getRequestIDsForBlock`

Returns a list with all request IDs in the block with block index `n`.

#### Parameters

| Name       | Type | Optional | Description                                            |
| ---------- | ---- | -------- | ------------------------------------------------------ |
| blockIndex | u32  | Yes      | The block index. The default value is the latest block |

#### Returns

| Name              | Type       | Description         |
| ----------------- | ---------- | ------------------- |
| blockIndex        | u32        | The block Index     |
| requestIDsInBlock | [[u8; 32]] | The ISC Request IDs |

### `getRequestReceipt`

Returns the receipt for the request with the given ID.

#### Parameters

| Name      | Type     | Optional | Description    |
| --------- | -------- | -------- | -------------- |
| requestID | [u8; 32] | No       | The request ID |

#### Returns

| Name           | Type                                        | Description         |
| -------------- | ------------------------------------------- | ------------------- |
| requestReceipt | [RequestReceipt](./types.md#requestreceipt) | The request receipt |

### `getRequestReceiptsForBlock`

Returns all the receipts in the block with index `blockIndex`.

#### Parameters

| Name       | Type | Optional | Description                                   |
| ---------- | ---- | -------- | --------------------------------------------- |
| blockIndex | u32  | Yes      | The block index. Defaults to the latest block |

#### Response

| Name            | Type                                                          | Description                  |
| --------------- | ------------------------------------------------------------- | ---------------------------- |
| requestReceipts | [RequestReceiptsResponse](./types.md#requestreceiptsresponse) | The request receipt response |

### `isRequestProcessed`

Returns whether the request with ID `u` has been processed.

#### Parameters

| Name      | Type     | Optional | Description    |
| --------- | -------- | -------- | -------------- |
| requestID | [u8; 32] | No       | The request ID |

#### Returns

| Name        | Type | Description                              |
| ----------- | ---- | ---------------------------------------- |
| isProcessed | bool | Whether the request was processed or not |

### `getEventsForRequest`

Returns the list of events triggered during the execution of the request with ID `requestID`.

### Parameters

| Name      | Type     | Optional | Description    |
| --------- | -------- | -------- | -------------- |
| requestID | [u8; 32] | No       | The request ID |

#### Returns

| Name   | Type                        | Description    |
| ------ | --------------------------- | -------------- |
| events | [[Event](./types.md#event)] | List of events |

### `getEventsForBlock`

Returns the list of events triggered during the execution of all requests in the block with index `blockIndex`.

#### Parameters

| Name       | Type | Optional | Description                                   |
| ---------- | ---- | -------- | --------------------------------------------- |
| blockIndex | u32  | Yes      | The block index. Defaults to the latest block |

#### Returns

| Name       | Type                        | Description     |
| ---------- | --------------------------- | --------------- |
| blockIndex | u32                         | The block index |
| events     | [[Event](./types.md#event)] | List of events  |
