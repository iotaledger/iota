# Interface: IIotaIdentityClient

[identity\_wasm](../modules/identity_wasm.md).IIotaIdentityClient

Helper interface necessary for `IotaIdentityClientExt`.

## Implemented by

- [`IotaIdentityClient`](../classes/iota_identity_client.IotaIdentityClient.md)

## Table of contents

### Methods

- [getAliasOutput](identity_wasm.IIotaIdentityClient.md#getaliasoutput)
- [getProtocolParameters](identity_wasm.IIotaIdentityClient.md#getprotocolparameters)

## Methods

### getAliasOutput

▸ **getAliasOutput**(`aliasId`): `Promise`\<[`string`, `AliasOutput`]\>

Resolve an Alias identifier, returning its latest `OutputId` and `AliasOutput`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `aliasId` | `string` |

#### Returns

`Promise`\<[`string`, `AliasOutput`]\>

___

### getProtocolParameters

▸ **getProtocolParameters**(): `Promise`\<`INodeInfoProtocol`\>

Returns the protocol parameters.

#### Returns

`Promise`\<`INodeInfoProtocol`\>
