# Class: IotaClient

## Constructors

### new IotaClient()

> **new IotaClient**(`options`): [`IotaClient`](IotaClient.md)

Establish a connection to a IOTA RPC endpoint

#### Parameters

• **options**: `NetworkOrTransport`

configuration options for the API Client

#### Returns

[`IotaClient`](IotaClient.md)

## Properties

### transport

> `protected` **transport**: [`IotaTransport`](../interfaces/IotaTransport.md)

## Methods

### getRpcApiVersion()

> **getRpcApiVersion**(): `Promise`\<`undefined` \| `string`\>

#### Returns

`Promise`\<`undefined` \| `string`\>

---

### getCoins()

> **getCoins**(`input`): `Promise`\<[`PaginatedCoins`](../interfaces/PaginatedCoins.md)\>

Get all Coin<`coin_type`> objects owned by an address.

#### Parameters

• **input**: [`GetCoinsParams`](../interfaces/GetCoinsParams.md)

#### Returns

`Promise`\<[`PaginatedCoins`](../interfaces/PaginatedCoins.md)\>

---

### getAllCoins()

> **getAllCoins**(`input`): `Promise`\<[`PaginatedCoins`](../interfaces/PaginatedCoins.md)\>

Get all Coin objects owned by an address.

#### Parameters

• **input**: [`GetAllCoinsParams`](../interfaces/GetAllCoinsParams.md)

#### Returns

`Promise`\<[`PaginatedCoins`](../interfaces/PaginatedCoins.md)\>

---

### getBalance()

> **getBalance**(`input`): `Promise`\<[`CoinBalance`](../type-aliases/CoinBalance.md)\>

Get the total coin balance for one coin type, owned by the address owner.

#### Parameters

• **input**: [`GetBalanceParams`](../interfaces/GetBalanceParams.md)

#### Returns

`Promise`\<[`CoinBalance`](../type-aliases/CoinBalance.md)\>

---

### getAllBalances()

> **getAllBalances**(`input`): `Promise`\<[`CoinBalance`](../type-aliases/CoinBalance.md)[]\>

Get the total coin balance for all coin types, owned by the address owner.

#### Parameters

• **input**: [`GetAllBalancesParams`](../interfaces/GetAllBalancesParams.md)

#### Returns

`Promise`\<[`CoinBalance`](../type-aliases/CoinBalance.md)[]\>

---

### getCoinMetadata()

> **getCoinMetadata**(`input`): `Promise`\<`null` \| [`CoinMetadata`](../interfaces/CoinMetadata.md)\>

Fetch CoinMetadata for a given coin type

#### Parameters

• **input**: [`GetCoinMetadataParams`](../interfaces/GetCoinMetadataParams.md)

#### Returns

`Promise`\<`null` \| [`CoinMetadata`](../interfaces/CoinMetadata.md)\>

---

### getTotalSupply()

> **getTotalSupply**(`input`): `Promise`\<[`CoinSupply`](../interfaces/CoinSupply.md)\>

Fetch total supply for a coin

#### Parameters

• **input**: [`GetTotalSupplyParams`](../interfaces/GetTotalSupplyParams.md)

#### Returns

`Promise`\<[`CoinSupply`](../interfaces/CoinSupply.md)\>

---

### call()

> **call**\<`T`\>(`method`, `params`): `Promise`\<`T`\>

Invoke any RPC method

#### Type Parameters

• **T** = `unknown`

#### Parameters

• **method**: `string`

the method to be invoked

• **params**: `unknown`[]

#### Returns

`Promise`\<`T`\>

---

### getMoveFunctionArgTypes()

> **getMoveFunctionArgTypes**(`input`): `Promise`\<[`IotaMoveFunctionArgType`](../type-aliases/IotaMoveFunctionArgType.md)[]\>

Get Move function argument types like read, write and full access

#### Parameters

• **input**: [`GetMoveFunctionArgTypesParams`](../interfaces/GetMoveFunctionArgTypesParams.md)

#### Returns

`Promise`\<[`IotaMoveFunctionArgType`](../type-aliases/IotaMoveFunctionArgType.md)[]\>

---

### getNormalizedMoveModulesByPackage()

> **getNormalizedMoveModulesByPackage**(`input`): `Promise`\<[`IotaMoveNormalizedModules`](../type-aliases/IotaMoveNormalizedModules.md)\>

Get a map from module name to
structured representations of Move modules

#### Parameters

• **input**: [`GetNormalizedMoveModulesByPackageParams`](../interfaces/GetNormalizedMoveModulesByPackageParams.md)

#### Returns

`Promise`\<[`IotaMoveNormalizedModules`](../type-aliases/IotaMoveNormalizedModules.md)\>

---

### getNormalizedMoveModule()

> **getNormalizedMoveModule**(`input`): `Promise`\<[`IotaMoveNormalizedModule`](../interfaces/IotaMoveNormalizedModule.md)\>

Get a structured representation of Move module

#### Parameters

• **input**: [`GetNormalizedMoveModuleParams`](../interfaces/GetNormalizedMoveModuleParams.md)

#### Returns

`Promise`\<[`IotaMoveNormalizedModule`](../interfaces/IotaMoveNormalizedModule.md)\>

---

### getNormalizedMoveFunction()

> **getNormalizedMoveFunction**(`input`): `Promise`\<[`IotaMoveNormalizedFunction`](../interfaces/IotaMoveNormalizedFunction.md)\>

Get a structured representation of Move function

#### Parameters

• **input**: [`GetNormalizedMoveFunctionParams`](../interfaces/GetNormalizedMoveFunctionParams.md)

#### Returns

`Promise`\<[`IotaMoveNormalizedFunction`](../interfaces/IotaMoveNormalizedFunction.md)\>

---

### getNormalizedMoveStruct()

> **getNormalizedMoveStruct**(`input`): `Promise`\<[`IotaMoveNormalizedStruct`](../interfaces/IotaMoveNormalizedStruct.md)\>

Get a structured representation of Move struct

#### Parameters

• **input**: [`GetNormalizedMoveStructParams`](../interfaces/GetNormalizedMoveStructParams.md)

#### Returns

`Promise`\<[`IotaMoveNormalizedStruct`](../interfaces/IotaMoveNormalizedStruct.md)\>

---

### getOwnedObjects()

> **getOwnedObjects**(`input`): `Promise`\<[`PaginatedObjectsResponse`](../interfaces/PaginatedObjectsResponse.md)\>

Get all objects owned by an address

#### Parameters

• **input**: [`GetOwnedObjectsParams`](../type-aliases/GetOwnedObjectsParams.md)

#### Returns

`Promise`\<[`PaginatedObjectsResponse`](../interfaces/PaginatedObjectsResponse.md)\>

---

### getObject()

> **getObject**(`input`): `Promise`\<[`IotaObjectResponse`](../interfaces/IotaObjectResponse.md)\>

Get details about an object

#### Parameters

• **input**: [`GetObjectParams`](../interfaces/GetObjectParams.md)

#### Returns

`Promise`\<[`IotaObjectResponse`](../interfaces/IotaObjectResponse.md)\>

---

### tryGetPastObject()

> **tryGetPastObject**(`input`): `Promise`\<[`ObjectRead`](../type-aliases/ObjectRead.md)\>

#### Parameters

• **input**: [`TryGetPastObjectParams`](../interfaces/TryGetPastObjectParams.md)

#### Returns

`Promise`\<[`ObjectRead`](../type-aliases/ObjectRead.md)\>

---

### multiGetObjects()

> **multiGetObjects**(`input`): `Promise`\<[`IotaObjectResponse`](../interfaces/IotaObjectResponse.md)[]\>

Batch get details about a list of objects. If any of the object ids are duplicates the call will fail

#### Parameters

• **input**: [`MultiGetObjectsParams`](../interfaces/MultiGetObjectsParams.md)

#### Returns

`Promise`\<[`IotaObjectResponse`](../interfaces/IotaObjectResponse.md)[]\>

---

### queryTransactionBlocks()

> **queryTransactionBlocks**(`input`): `Promise`\<[`PaginatedTransactionResponse`](../interfaces/PaginatedTransactionResponse.md)\>

Get transaction blocks for a given query criteria

#### Parameters

• **input**: [`QueryTransactionBlocksParams`](../type-aliases/QueryTransactionBlocksParams.md)

#### Returns

`Promise`\<[`PaginatedTransactionResponse`](../interfaces/PaginatedTransactionResponse.md)\>

---

### getTransactionBlock()

> **getTransactionBlock**(`input`): `Promise`\<[`IotaTransactionBlockResponse`](../interfaces/IotaTransactionBlockResponse.md)\>

#### Parameters

• **input**: [`GetTransactionBlockParams`](../interfaces/GetTransactionBlockParams.md)

#### Returns

`Promise`\<[`IotaTransactionBlockResponse`](../interfaces/IotaTransactionBlockResponse.md)\>

---

### multiGetTransactionBlocks()

> **multiGetTransactionBlocks**(`input`): `Promise`\<[`IotaTransactionBlockResponse`](../interfaces/IotaTransactionBlockResponse.md)[]\>

#### Parameters

• **input**: [`MultiGetTransactionBlocksParams`](../interfaces/MultiGetTransactionBlocksParams.md)

#### Returns

`Promise`\<[`IotaTransactionBlockResponse`](../interfaces/IotaTransactionBlockResponse.md)[]\>

---

### executeTransactionBlock()

> **executeTransactionBlock**(`__namedParameters`): `Promise`\<[`IotaTransactionBlockResponse`](../interfaces/IotaTransactionBlockResponse.md)\>

#### Parameters

• **\_\_namedParameters**: [`ExecuteTransactionBlockParams`](../interfaces/ExecuteTransactionBlockParams.md)

#### Returns

`Promise`\<[`IotaTransactionBlockResponse`](../interfaces/IotaTransactionBlockResponse.md)\>

---

### signAndExecuteTransaction()

> **signAndExecuteTransaction**(`__namedParameters`): `Promise`\<[`IotaTransactionBlockResponse`](../interfaces/IotaTransactionBlockResponse.md)\>

#### Parameters

• **\_\_namedParameters**: `object` & `Omit`\<[`ExecuteTransactionBlockParams`](../interfaces/ExecuteTransactionBlockParams.md), `"signature"` \| `"transactionBlock"`\>

#### Returns

`Promise`\<[`IotaTransactionBlockResponse`](../interfaces/IotaTransactionBlockResponse.md)\>

---

### getTotalTransactionBlocks()

> **getTotalTransactionBlocks**(): `Promise`\<`bigint`\>

Get total number of transactions

#### Returns

`Promise`\<`bigint`\>

---

### getReferenceGasPrice()

> **getReferenceGasPrice**(): `Promise`\<`bigint`\>

Getting the reference gas price for the network

#### Returns

`Promise`\<`bigint`\>

---

### getStakes()

> **getStakes**(`input`): `Promise`\<[`DelegatedStake`](../interfaces/DelegatedStake.md)[]\>

Return the delegated stakes for an address

#### Parameters

• **input**: [`GetStakesParams`](../interfaces/GetStakesParams.md)

#### Returns

`Promise`\<[`DelegatedStake`](../interfaces/DelegatedStake.md)[]\>

---

### getTimelockedStakes()

> **getTimelockedStakes**(`input`): `Promise`\<[`DelegatedTimelockedStake`](../interfaces/DelegatedTimelockedStake.md)[]\>

Return the timelocked delegated stakes for an address

#### Parameters

• **input**: [`GetTimelockedStakesParams`](../interfaces/GetTimelockedStakesParams.md)

#### Returns

`Promise`\<[`DelegatedTimelockedStake`](../interfaces/DelegatedTimelockedStake.md)[]\>

---

### getStakesByIds()

> **getStakesByIds**(`input`): `Promise`\<[`DelegatedStake`](../interfaces/DelegatedStake.md)[]\>

Return the delegated stakes queried by id.

#### Parameters

• **input**: [`GetStakesByIdsParams`](../interfaces/GetStakesByIdsParams.md)

#### Returns

`Promise`\<[`DelegatedStake`](../interfaces/DelegatedStake.md)[]\>

---

### getTimelockedStakesByIds()

> **getTimelockedStakesByIds**(`input`): `Promise`\<[`DelegatedTimelockedStake`](../interfaces/DelegatedTimelockedStake.md)[]\>

Return the timelocked delegated stakes queried by id.

#### Parameters

• **input**: [`GetTimelockedStakesByIdsParams`](../interfaces/GetTimelockedStakesByIdsParams.md)

#### Returns

`Promise`\<[`DelegatedTimelockedStake`](../interfaces/DelegatedTimelockedStake.md)[]\>

---

### getLatestIotaSystemState()

> **getLatestIotaSystemState**(): `Promise`\<[`IotaSystemStateSummary`](../interfaces/IotaSystemStateSummary.md)\>

Return the latest system state content.

#### Returns

`Promise`\<[`IotaSystemStateSummary`](../interfaces/IotaSystemStateSummary.md)\>

---

### queryEvents()

> **queryEvents**(`input`): `Promise`\<[`PaginatedEvents`](../interfaces/PaginatedEvents.md)\>

Get events for a given query criteria

#### Parameters

• **input**: [`QueryEventsParams`](../interfaces/QueryEventsParams.md)

#### Returns

`Promise`\<[`PaginatedEvents`](../interfaces/PaginatedEvents.md)\>

---

### ~~subscribeEvent()~~

> **subscribeEvent**(`input`): `Promise`\<[`Unsubscribe`](../type-aliases/Unsubscribe.md)\>

Subscribe to get notifications whenever an event matching the filter occurs

#### Parameters

• **input**: [`SubscribeEventParams`](../interfaces/SubscribeEventParams.md) & `object`

#### Returns

`Promise`\<[`Unsubscribe`](../type-aliases/Unsubscribe.md)\>

#### Deprecated

---

### ~~subscribeTransaction()~~

> **subscribeTransaction**(`input`): `Promise`\<[`Unsubscribe`](../type-aliases/Unsubscribe.md)\>

#### Parameters

• **input**: [`SubscribeTransactionParams`](../interfaces/SubscribeTransactionParams.md) & `object`

#### Returns

`Promise`\<[`Unsubscribe`](../type-aliases/Unsubscribe.md)\>

#### Deprecated

---

### devInspectTransactionBlock()

> **devInspectTransactionBlock**(`input`): `Promise`\<[`DevInspectResults`](../interfaces/DevInspectResults.md)\>

Runs the transaction block in dev-inspect mode. Which allows for nearly any
transaction (or Move call) with any arguments. Detailed results are
provided, including both the transaction effects and any return values.

#### Parameters

• **input**: [`DevInspectTransactionBlockParams`](../interfaces/DevInspectTransactionBlockParams.md)

#### Returns

`Promise`\<[`DevInspectResults`](../interfaces/DevInspectResults.md)\>

---

### dryRunTransactionBlock()

> **dryRunTransactionBlock**(`input`): `Promise`\<[`DryRunTransactionBlockResponse`](../interfaces/DryRunTransactionBlockResponse.md)\>

Dry run a transaction block and return the result.

#### Parameters

• **input**: [`DryRunTransactionBlockParams`](../interfaces/DryRunTransactionBlockParams.md)

#### Returns

`Promise`\<[`DryRunTransactionBlockResponse`](../interfaces/DryRunTransactionBlockResponse.md)\>

---

### getDynamicFields()

> **getDynamicFields**(`input`): `Promise`\<[`DynamicFieldPage`](../type-aliases/DynamicFieldPage.md)\>

Return the list of dynamic field objects owned by an object

#### Parameters

• **input**: [`GetDynamicFieldsParams`](../interfaces/GetDynamicFieldsParams.md)

#### Returns

`Promise`\<[`DynamicFieldPage`](../type-aliases/DynamicFieldPage.md)\>

---

### getDynamicFieldObject()

> **getDynamicFieldObject**(`input`): `Promise`\<[`IotaObjectResponse`](../interfaces/IotaObjectResponse.md)\>

Return the dynamic field object information for a specified object

#### Parameters

• **input**: [`GetDynamicFieldObjectParams`](../interfaces/GetDynamicFieldObjectParams.md)

#### Returns

`Promise`\<[`IotaObjectResponse`](../interfaces/IotaObjectResponse.md)\>

---

### getLatestCheckpointSequenceNumber()

> **getLatestCheckpointSequenceNumber**(): `Promise`\<`string`\>

Get the sequence number of the latest checkpoint that has been executed

#### Returns

`Promise`\<`string`\>

---

### getCheckpoint()

> **getCheckpoint**(`input`): `Promise`\<[`Checkpoint`](../interfaces/Checkpoint.md)\>

Returns information about a given checkpoint

#### Parameters

• **input**: [`GetCheckpointParams`](../interfaces/GetCheckpointParams.md)

#### Returns

`Promise`\<[`Checkpoint`](../interfaces/Checkpoint.md)\>

---

### getCheckpoints()

> **getCheckpoints**(`input`): `Promise`\<[`CheckpointPage`](../type-aliases/CheckpointPage.md)\>

Returns historical checkpoints paginated

#### Parameters

• **input**: [`PaginationArguments`](../interfaces/PaginationArguments.md)\<`null` \| `string`\> & [`GetCheckpointsParams`](../interfaces/GetCheckpointsParams.md)

#### Returns

`Promise`\<[`CheckpointPage`](../type-aliases/CheckpointPage.md)\>

---

### getCommitteeInfo()

> **getCommitteeInfo**(`input`?): `Promise`\<[`CommitteeInfo`](../interfaces/CommitteeInfo.md)\>

Return the committee information for the asked epoch

#### Parameters

• **input?**: [`GetCommitteeInfoParams`](../interfaces/GetCommitteeInfoParams.md)

#### Returns

`Promise`\<[`CommitteeInfo`](../interfaces/CommitteeInfo.md)\>

---

### getNetworkMetrics()

> **getNetworkMetrics**(): `Promise`\<[`NetworkMetrics`](../interfaces/NetworkMetrics.md)\>

#### Returns

`Promise`\<[`NetworkMetrics`](../interfaces/NetworkMetrics.md)\>

---

### getAddressMetrics()

> **getAddressMetrics**(): `Promise`\<[`AddressMetrics`](../interfaces/AddressMetrics.md)\>

#### Returns

`Promise`\<[`AddressMetrics`](../interfaces/AddressMetrics.md)\>

---

### getEpochMetrics()

> **getEpochMetrics**(`input`?): `Promise`\<[`EpochMetricsPage`](../type-aliases/EpochMetricsPage.md)\>

#### Parameters

• **input?**: `object` & [`PaginationArguments`](../interfaces/PaginationArguments.md)\<`null` \| `string`\>

#### Returns

`Promise`\<[`EpochMetricsPage`](../type-aliases/EpochMetricsPage.md)\>

---

### getAllEpochAddressMetrics()

> **getAllEpochAddressMetrics**(`input`?): `Promise`\<[`AllEpochsAddressMetrics`](../type-aliases/AllEpochsAddressMetrics.md)\>

#### Parameters

• **input?**

• **input.descendingOrder?**: `boolean`

#### Returns

`Promise`\<[`AllEpochsAddressMetrics`](../type-aliases/AllEpochsAddressMetrics.md)\>

---

### getCheckpointAddressMetrics()

> **getCheckpointAddressMetrics**(`input`?): `Promise`\<[`AddressMetrics`](../interfaces/AddressMetrics.md)\>

#### Parameters

• **input?**

• **input.checkpoint?**: `string`

#### Returns

`Promise`\<[`AddressMetrics`](../interfaces/AddressMetrics.md)\>

---

### getEpochs()

> **getEpochs**(`input`?): `Promise`\<[`EpochPage`](../type-aliases/EpochPage.md)\>

Return the committee information for the asked epoch

#### Parameters

• **input?**: `object` & [`PaginationArguments`](../interfaces/PaginationArguments.md)\<`null` \| `string`\>

#### Returns

`Promise`\<[`EpochPage`](../type-aliases/EpochPage.md)\>

---

### getMoveCallMetrics()

> **getMoveCallMetrics**(): `Promise`\<[`MoveCallMetrics`](../interfaces/MoveCallMetrics.md)\>

Returns list of top move calls by usage

#### Returns

`Promise`\<[`MoveCallMetrics`](../interfaces/MoveCallMetrics.md)\>

---

### getCurrentEpoch()

> **getCurrentEpoch**(): `Promise`\<[`EpochInfo`](../interfaces/EpochInfo.md)\>

Return the committee information for the asked epoch

#### Returns

`Promise`\<[`EpochInfo`](../interfaces/EpochInfo.md)\>

---

### getTotalTransactions()

> **getTotalTransactions**(): `Promise`\<`string`\>

#### Returns

`Promise`\<`string`\>

---

### getValidatorsApy()

> **getValidatorsApy**(): `Promise`\<[`ValidatorsApy`](../interfaces/ValidatorsApy.md)\>

Return the Validators APYs

#### Returns

`Promise`\<[`ValidatorsApy`](../interfaces/ValidatorsApy.md)\>

---

### getChainIdentifier()

> **getChainIdentifier**(): `Promise`\<`string`\>

#### Returns

`Promise`\<`string`\>

---

### getProtocolConfig()

> **getProtocolConfig**(`input`?): `Promise`\<[`ProtocolConfig`](../interfaces/ProtocolConfig.md)\>

#### Parameters

• **input?**: [`GetProtocolConfigParams`](../interfaces/GetProtocolConfigParams.md)

#### Returns

`Promise`\<[`ProtocolConfig`](../interfaces/ProtocolConfig.md)\>

---

### waitForTransaction()

> **waitForTransaction**(`__namedParameters`): `Promise`\<[`IotaTransactionBlockResponse`](../interfaces/IotaTransactionBlockResponse.md)\>

Wait for a transaction block result to be available over the API.
This can be used in conjunction with `executeTransactionBlock` to wait for the transaction to
be available via the API.
This currently polls the `getTransactionBlock` API to check for the transaction.

#### Parameters

• **\_\_namedParameters**: `object` & [`GetTransactionBlockParams`](../interfaces/GetTransactionBlockParams.md)

#### Returns

`Promise`\<[`IotaTransactionBlockResponse`](../interfaces/IotaTransactionBlockResponse.md)\>
