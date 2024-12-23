# Interface: IotaTransport

## Methods

### request()

> **request**\<`T`\>(`input`): `Promise`\<`T`\>

#### Type Parameters

• **T** = `unknown`

#### Parameters

• **input**: [`IotaTransportRequestOptions`](IotaTransportRequestOptions.md)

#### Returns

`Promise`\<`T`\>

---

### subscribe()

> **subscribe**\<`T`\>(`input`): `Promise`\<() => `Promise`\<`boolean`\>\>

#### Type Parameters

• **T** = `unknown`

#### Parameters

• **input**: [`IotaTransportSubscribeOptions`](IotaTransportSubscribeOptions.md)\<`T`\>

#### Returns

`Promise`\<() => `Promise`\<`boolean`\>\>
