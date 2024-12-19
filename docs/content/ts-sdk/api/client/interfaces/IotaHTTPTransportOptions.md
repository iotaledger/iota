# Interface: IotaHTTPTransportOptions

## Properties

### fetch()?

> `optional` **fetch**: (`input`, `init`?) => `Promise`\<`Response`\>(`input`, `init`?) => `Promise`\<`Response`\>

[MDN Reference](https://developer.mozilla.org/docs/Web/API/fetch)

#### Parameters

• **input**: `URL` \| `RequestInfo`

• **init?**: `RequestInit`

#### Returns

`Promise`\<`Response`\>

#### Parameters

• **input**: `string` \| `URL` \| `Request`

• **init?**: `RequestInit`

#### Returns

`Promise`\<`Response`\>

---

### WebSocketConstructor()?

> `optional` **WebSocketConstructor**: (`url`, `protocols`?) => `WebSocket`

#### Parameters

• **url**: `string` \| `URL`

• **protocols?**: `string` \| `string`[]

#### Returns

`WebSocket`

#### prototype

> **prototype**: `WebSocket`

#### CONNECTING

> `readonly` **CONNECTING**: `0`

#### OPEN

> `readonly` **OPEN**: `1`

#### CLOSING

> `readonly` **CLOSING**: `2`

#### CLOSED

> `readonly` **CLOSED**: `3`

---

### url

> **url**: `string`

---

### rpc?

> `optional` **rpc**: `object`

#### headers?

> `optional` **headers**: [`HttpHeaders`](../type-aliases/HttpHeaders.md)

#### url?

> `optional` **url**: `string`

---

### websocket?

> `optional` **websocket**: `WebsocketClientOptions` & `object`

#### Type declaration

##### url?

> `optional` **url**: `string`
