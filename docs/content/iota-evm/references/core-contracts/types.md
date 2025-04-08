# Types

To call core contracts from Move you have to encode the parameters using BCS. This page lists custom enums and structs used.

## Enum AgentID

| Name                   | Type                      |
|------------------------|---------------------------|
| NoType                 | null                      |
| AddressAgentID         | IscAddressAgentID         |
| ContractAgentID        | IscContractAgentID        |
| EthereumAddressAgentID | IscEthereumAddressAgentID |
| NilAgentID             | IscNilAgentID             |

## Struct IotaCoinInfo

| Name        | Type       |
|-------------|------------|
| CoinType    | [CoinType](#struct-cointype) |
| Decimals    | uint8                        |
| Name        | string                       |
| Symbol      | string                       |
| Description | string                       |
| IconURL     | string                       |
| TotalSupply | uint64                       |

## Struct CoinType

| Name | Type   |
|------|--------|
| s    | string |

## Map CoinBalances

| Name      | Type                         |
|-----------|------------------------------|
| CoinType  | [CoinType](#struct-cointype) |
| CoinValue | uint64                       |

## Struct BlockInfo

| Name                  | Type        |
|-----------------------|-------------|
| schemaVersion         | uint8       |
| blockIndex            | uint32      |
| timestamp             | uint64      |
| previousAnchor        | [StateAnchor](#struct-stateanchor) |
| l1Params              | [L1Params](#struct-l1params)    |
| totalRequests         | uint16      |
| numSuccessfulRequests | uint16      |
| numOffLedgerRequests  | uint16      |
| gasBurned             | uint64      |
| gasFeeCharged         | uint64      |

## Struct RequestReceiptsResponse

| Name       | Type                 |
|------------|----------------------|
| BlockIndex | uint32               |
| Receipts   | [RequestReceipt]     |

## Struct RequestReceipt

| Name          | Type                        |
|---------------|-----------------------------|
| Request       | [Request](#enum-request)    |
| Error         | [IscUnresolvedVMError](#struct-unresolvedvmerror) (optional) |
| GasBudget     | uint64                      |
| GasBurned     | uint64                      |
| GasFeeCharged | uint64                      |
| GasBurnLog    | [GasBurnLog](#struct-gasburnlog) |
| BlockIndex    | uint32                      |
| RequestIndex  | uint16                      |

## Struct UnresolvedVMError

| Name      | Type                                  |
|-----------|---------------------------------------|
| errorCode | [IscVMErrorCode](#struct-vmerrorcode) |
| params    | [[VMErrorParam](#enum-vmerrorparam)]  |

## Struct VMErrorCode

| Name       | Type   |
|------------|--------|
| contractID | uint32 |
| iD         | uint16 |

## Struct GasBurnLog

| Name    | Type                                     |
|---------|------------------------------------------|
| records | [[GasBurnRecord](#struct-gasburnrecord)] |

## Struct GasBurnRecord

| Name      | Type   |
|-----------|--------|
| code      | uint16 |
| gasBurned | uint64 |

## Enum VMErrorParam

| Name    | Type       |
|---------|------------|
| NoType  | null       |
| uint16  | uint16     |
| int64   | [uint8; 8] |
| string  | string     |
| uint8   | uint8      |
| int32   | [uint8; 4] |
| uint64  | uint64     |
| int16   | [uint8; 2] |
| uint32  | uint32     |

## Enum Request

| Name    | Type    |
|---------|---------|
| OnLedger | [OnLedgerRequestData](#struct-onledgerrequestdata) |
| OffLedger | OffLedgerRequestData |
| EVMOffLedgerTx | evmOffLedgerTxRequest |

## Struct OnLedgerRequestData

| Name             | Type                     |
|------------------|--------------------------|
| requestRef       | [ObjectRef](#struct-objectref)         |
| senderAddress    | *[u8; 32]       |
| targetAddress    | *[u8; 32]       |
| assets           | *[Assets](#struct-assets)                  |
| assetsBag        | *[AssetsBagWithBalances](#struct-assetsbagwithbalances)       |
| requestMetadata  | *[RequestMetadata](#struct-requestmetadata)         |

## Struct ObjectRef

| Name     | Type                |
|----------|---------------------|
| objectID | [u8; 32]           |
| version  | u64                |
| digest   | [u8]            |

## Struct Assets

| Name    | Type                              |
|---------|-----------------------------------|
| Coins   | [CoinBalances](#map-coinbalances) |
| Objects | [u8; 32]                          |

## Struct AssetsBagWithBalances

| Name       | Type                     |
|------------|--------------------------|
| AssetsBag  | [AssetsBag](#struct-assetsbag) |
| Assets     | [Assets](#struct-assets)       |

## Struct AssetsBag

| Name | Type       |
|------|------------|
| iD   | [u8; 32]   |
| size | u64        |

## Struct RequestMetadata

| Name            | Type                              |
|-----------------|-----------------------------------|
| senderContract  | [ContractIdentity](#struct-contractidentity) |
| message         | [Message](#struct-message)       |
| allowance       | *[Assets](#struct-assets)        |
| gasBudget       | uint64                           |

## Struct ContractIdentity

| Name    | Type                          |
|---------|-------------------------------|
| kind    | u8          |
| evmAddr | [u8, 20]                      |
| hname   | uint32        |

## Struct Message

| Name   | Type                     |
|--------|--------------------------|
| Target | [CallTarget](#struct-calltarget) |
| Params | [[byte]] |

## Struct CallTarget

| Name       | Type |
|------------|------|
| contract   | u32  |
| entryPoint | u32  |

## Struct L1Params

| Name       | Type              |
|------------|-------------------|
| protocol   | [Protocol](#struct-protocol) |
| baseToken  | [IotaCoinInfo](#struct-iotacoininfo) |

## Struct Protocol

| Name                  | Type              |
|-----------------------|-------------------|
| epoch                 | BigInt |
| protocolVersion       | BigInt |
| systemStateVersion    | BigInt |
| iotaTotalSupply       | BigInt |
| referenceGasPrice     | BigInt |
| epochStartTimestampMs | BigInt |
| epochDurationMs       | BigInt |

## Struct StateAnchor

| Name       | Type                     |
|------------|--------------------------|
| anchor     | [AnchorWithRef](#struct-anchorwithref) |
| iscPackage | [u8; 32]                |

## Struct AnchorWithRef

| Name       | Type                |
|------------|---------------------|
| objectRef  | [ObjectRef](#struct-objectref)     |
| object     | [Anchor](#struct-anchor)       |
| owner      | [u8; 32]           |

## Struct Anchor

| Name          | Type                     |
|---------------|--------------------------|
| iD            | [u8; 32]                |
| assets        | [Referent_AssetsBag](#struct-referent_assetsbag) |
| stateMetadata | [u8]                 |
| stateIndex    | u32                     |

## Struct Referent_AssetsBag

| Name  | Type               | optional |
|-------|--------------------|----------|
| iD    | [u8; 32]          | No  |
| value | [AssetsBag](#struct-assetsbag)  | Yes |

## Struct Event

| Name       | Type   |
|------------|--------|
| ContractID | u32    |
| Topic      | string |
| Timestamp  | u64    |
| Payload    | [u8]   |

## Struct FeePolicy

| Name              | Type                        |
|-------------------|-----------------------------|
| eVMGasRatio       | [Ratio32](#struct-ratio32)  |
| gasPerToken       | [Ratio32](#struct-ratio32)  |
| validatorFeeShare | u8                          |

## Struct Ratio32

| Name | Type   |
|------|--------|
| A    | uint32 |
| B    | uint32 |

## Struct Limits

| Name                   | Type |
|------------------------|------|
| maxGasPerBlock         | u64  |
| minGasPerRequest       | u64  |
| maxGasPerRequest       | u64  |
| maxGasExternalViewCall | u64  |

## Struct ChainInfo

| Name            | Type                                               |
|-----------------|----------------------------------------------------|
| chainID         | [u8; 32]                                           |
| chainOwnerID    | [u8; 32]                                           |
| gasFeePolicy    | [FeePolicy](#struct-feepolicy)                     |
| gasLimits       | [Limits](#struct-limits)                           |
| blockKeepAmount | [u8; 4]                                            |
| publicURL       | string                                             |
| metadata        | [PublicChainMetadata](#struct-publicchainmetadata) |

## Struct PublicChainMetadata

| Name            | Type   |
|-----------------|--------|
| eVMJsonRPCURL   | string |
| eVMWebSocketURL | string |
| name            | string |
| description     | string |
| website         | string |

## Struct AccessNodeInfo

| Name           | Type                                             |
|----------------|--------------------------------------------------|
| NodePubKey     | [u8; 32] |
| AccessNodeData | [AccessNodeData](#struct-accessnodedata)         |

## Struct AccessNodeData

| Name          | Type     |
|---------------|----------|
| ValidatorAddr | [u8; 32] |
| Certificate   | [u8]     |
| ForCommittee  | bool     |
| AccessAPI     | string   |

## Struct ContractRecord

| Name | Type   |
|------|--------|
| Name | string |
