# Interface: IotaGraphQLClientOptions\<Queries\>

## Type Parameters

• **Queries** _extends_ `Record`\<`string`, [`GraphQLDocument`](../type-aliases/GraphQLDocument.md)\>

## Properties

### url

> **url**: `string`

---

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

### headers?

> `optional` **headers**: `Record`\<`string`, `string`\>

---

### queries?

> `optional` **queries**: `Queries`
