# Types

To call core contracts from Move you have to encode the parameters using BCS. This page lists custom enums and structs used.

## Enums

### `AgentID`

| Name                   | Type                      |
| ---------------------- | ------------------------- |
| NoType                 | null                      |
| AddressAgentID         | IscAddressAgentID         |
| ContractAgentID        | IscContractAgentID        |
| EthereumAddressAgentID | IscEthereumAddressAgentID |
| NilAgentID             | IscNilAgentID             |

### `Request`

| Name           | Type                                        |
| -------------- | ------------------------------------------- |
| OnLedger       | [OnLedgerRequestData](#onledgerrequestdata) |
| OffLedger      | OffLedgerRequestData                        |
| EVMOffLedgerTx | evmOffLedgerTxRequest                       |

### `VMErrorParam`

| Name   | Type    |
| ------ | ------- |
| NoType | null    |
| uint16 | u16     |
| int64  | [u8; 8] |
| string | string  |
| uint8  | u8      |
| int32  | [u8; 4] |
| uint64 | u64     |
| int16  | [u8; 2] |
| uint32 | u32     |

## Maps

### CoinBalances

[[CoinType](#cointype)][u64]

## Structs

### `AccessNodeData`

| Name          | Type     |
| ------------- | -------- |
| ValidatorAddr | [u8; 32] |
| Certificate   | [u8]     |
| ForCommittee  | bool     |
| AccessAPI     | string   |

### `AccessNodeInfo`

| Name           | Type                              |
| -------------- | --------------------------------- |
| NodePubKey     | [u8; 32]                          |
| AccessNodeData | [AccessNodeData](#accessnodedata) |

### `Anchor`

| Name          | Type                                      |
| ------------- | ----------------------------------------- |
| iD            | [u8; 32]                                  |
| assets        | [Referent_AssetsBag](#referent_assetsbag) |
| stateMetadata | [u8]                                      |
| stateIndex    | u32                                       |

### `AnchorWithRef`

| Name      | Type                    |
| --------- | ----------------------- |
| objectRef | [ObjectRef](#objectref) |
| object    | [Anchor](#anchor)       |
| owner     | [u8; 32]                |

### `Assets`

| Name    | Type                          |
| ------- | ----------------------------- |
| Coins   | [CoinBalances](#coinbalances) |
| Objects | [u8; 32]                      |

### `AssetsBag`

| Name | Type     |
| ---- | -------- |
| iD   | [u8; 32] |
| size | u64      |

### `AssetsBagWithBalances`

| Name      | Type                    |
| --------- | ----------------------- |
| AssetsBag | [AssetsBag](#assetsbag) |
| Assets    | [Assets](#assets)       |

### `BlockInfo`

| Name                  | Type                        |
| --------------------- | --------------------------- |
| schemaVersion         | u8                          |
| blockIndex            | u32                         |
| timestamp             | u64                         |
| previousAnchor        | [StateAnchor](#stateanchor) |
| l1Params              | [L1Params](#l1params)       |
| totalRequests         | u16                         |
| numSuccessfulRequests | u16                         |
| numOffLedgerRequests  | u16                         |
| gasBurned             | u64                         |
| gasFeeCharged         | u64                         |

### `CallTarget`

| Name       | Type |
| ---------- | ---- |
| contract   | u32  |
| entryPoint | u32  |

### `ChainInfo`

| Name            | Type                                        |
| --------------- | ------------------------------------------- |
| chainID         | [u8; 32]                                    |
| chainOwnerID    | [u8; 32]                                    |
| gasFeePolicy    | [FeePolicy](#feepolicy)                     |
| gasLimits       | [Limits](#limits)                           |
| blockKeepAmount | [u8; 4]                                     |
| publicURL       | string                                      |
| metadata        | [PublicChainMetadata](#publicchainmetadata) |

### `CoinType`

| Name | Type   |
| ---- | ------ |
| s    | string |

### `ContractIdentity`

| Name    | Type     |
| ------- | -------- |
| kind    | u8       |
| evmAddr | [u8, 20] |
| hname   | u32      |

### `ContractRecord`

| Name | Type   |
| ---- | ------ |
| Name | string |

### `Event`

| Name       | Type   |
| ---------- | ------ |
| ContractID | u32    |
| Topic      | string |
| Timestamp  | u64    |
| Payload    | [u8]   |

### `FeePolicy`

| Name              | Type                |
| ----------------- | ------------------- |
| eVMGasRatio       | [Ratio32](#ratio32) |
| gasPerToken       | [Ratio32](#ratio32) |
| validatorFeeShare | u8                  |

### `GasBurnLog`

| Name    | Type                              |
| ------- | --------------------------------- |
| records | [[GasBurnRecord](#gasburnrecord)] |

### `GasBurnRecord`

| Name      | Type |
| --------- | ---- |
| code      | u16  |
| gasBurned | u64  |

### `IotaCoinInfo`

| Name        | Type                  |
| ----------- | --------------------- |
| CoinType    | [CoinType](#cointype) |
| Decimals    | u8                    |
| Name        | string                |
| Symbol      | string                |
| Description | string                |
| IconURL     | string                |
| TotalSupply | u64                   |

### `L1Params`

| Name      | Type                          |
| --------- | ----------------------------- |
| protocol  | [Protocol](#protocol)         |
| baseToken | [IotaCoinInfo](#iotacoininfo) |

### `Limits`

| Name                   | Type |
| ---------------------- | ---- |
| maxGasPerBlock         | u64  |
| minGasPerRequest       | u64  |
| maxGasPerRequest       | u64  |
| maxGasExternalViewCall | u64  |

### `Message`

| Name   | Type                      |
| ------ | ------------------------- |
| Target | [CallTarget](#calltarget) |
| Params | [[byte]]                  |

### `ObjectRef`

| Name     | Type     |
| -------- | -------- |
| objectID | [u8; 32] |
| version  | u64      |
| digest   | [u8]     |

### `OnLedgerRequestData`

| Name            | Type                                             |
| --------------- | ------------------------------------------------ |
| requestRef      | [ObjectRef](#objectref)                          |
| senderAddress   | *[u8; 32]                                        |
| targetAddress   | *[u8; 32]                                        |
| assets          | *[Assets](#assets)                               |
| assetsBag       | *[AssetsBagWithBalances](#assetsbagwithbalances) |
| requestMetadata | *[RequestMetadata](#requestmetadata)             |

### `Protocol`

| Name                  | Type   |
| --------------------- | ------ |
| epoch                 | BigInt |
| protocolVersion       | BigInt |
| systemStateVersion    | BigInt |
| iotaTotalSupply       | BigInt |
| referenceGasPrice     | BigInt |
| epochStartTimestampMs | BigInt |
| epochDurationMs       | BigInt |

### `PublicChainMetadata`

| Name            | Type   |
| --------------- | ------ |
| eVMJsonRPCURL   | string |
| eVMWebSocketURL | string |
| name            | string |
| description     | string |
| website         | string |

### `Ratio32`

| Name | Type |
| ---- | ---- |
| A    | u32  |
| B    | u32  |

### `Referent_AssetsBag`

| Name  | Type                    | optional |
| ----- | ----------------------- | -------- |
| iD    | [u8; 32]                | No       |
| value | [AssetsBag](#assetsbag) | Yes      |

### `RequestMetadata`

| Name           | Type                                  |
| -------------- | ------------------------------------- |
| senderContract | [ContractIdentity](#contractidentity) |
| message        | [Message](#message)                   |
| allowance      | *[Assets](#assets)                    |
| gasBudget      | u64                                   |

### `RequestReceipt`

| Name          | Type                                                  |
| ------------- | ----------------------------------------------------- |
| Request       | [Request](#request)                                   |
| Error         | [IscUnresolvedVMError](#unresolvedvmerror) (optional) |
| GasBudget     | u64                                                   |
| GasBurned     | u64                                                   |
| GasFeeCharged | u64                                                   |
| GasBurnLog    | [GasBurnLog](#gasburnlog)                             |
| BlockIndex    | u32                                                   |
| RequestIndex  | u16                                                   |

### `RequestReceiptsResponse`

| Name       | Type             |
| ---------- | ---------------- |
| BlockIndex | u32              |
| Receipts   | [RequestReceipt] |

### `StateAnchor`

| Name       | Type                            |
| ---------- | ------------------------------- |
| anchor     | [AnchorWithRef](#anchorwithref) |
| iscPackage | [u8; 32]                        |

### `UnresolvedVMError`

| Name      | Type                            |
| --------- | ------------------------------- |
| errorCode | [IscVMErrorCode](#vmerrorcode)  |
| params    | [[VMErrorParam](#vmerrorparam)] |

### `VMErrorCode`

| Name       | Type |
| ---------- | ---- |
| contractID | u32  |
| iD         | u16  |
