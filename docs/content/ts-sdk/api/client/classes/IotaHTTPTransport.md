# Class: IotaHTTPTransport

## Implements

- [`IotaTransport`](../interfaces/IotaTransport.md)

## Constructors

### new IotaHTTPTransport()

> **new IotaHTTPTransport**(`options`): [`IotaHTTPTransport`](IotaHTTPTransport.md)

#### Parameters

• **options**: [`IotaHTTPTransportOptions`](../interfaces/IotaHTTPTransportOptions.md)

#### Returns

[`IotaHTTPTransport`](IotaHTTPTransport.md)

## Methods

### fetch()

> **fetch**(`input`, `init`?): `Promise`\<`Response`\>

#### Parameters

• **input**: `RequestInfo`

• **init?**: `RequestInit`

#### Returns

`Promise`\<`Response`\>

---

### request()

> **request**\<`T`\>(`input`): `Promise`\<`T`\>

#### Type Parameters

• **T**

#### Parameters

• **input**: [`IotaTransportRequestOptions`](../interfaces/IotaTransportRequestOptions.md)

#### Returns

`Promise`\<`T`\>

#### Implementation of

[`IotaTransport`](../interfaces/IotaTransport.md).[`request`](../interfaces/IotaTransport.md#request)

---

### subscribe()

> **subscribe**\<`T`\>(`input`): `Promise`\<() => `Promise`\<`boolean`\>\>

#### Type Parameters

• **T**

#### Parameters

• **input**: [`IotaTransportSubscribeOptions`](../interfaces/IotaTransportSubscribeOptions.md)\<`T`\>

#### Returns

`Promise`\<() => `Promise`\<`boolean`\>\>

#### Implementation of

[`IotaTransport`](../interfaces/IotaTransport.md).[`subscribe`](../interfaces/IotaTransport.md#subscribe)
