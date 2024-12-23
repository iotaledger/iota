# Class: IotaGraphQLClient\<Queries\>

## Type Parameters

• **Queries** *extends* `Record`\<`string`, [`GraphQLDocument`](../type-aliases/GraphQLDocument.md)\> = `object`

## Constructors

### new IotaGraphQLClient()

> **new IotaGraphQLClient**\<`Queries`\>(`__namedParameters`): [`IotaGraphQLClient`](IotaGraphQLClient.md)\<`Queries`\>

#### Parameters

• **\_\_namedParameters**: [`IotaGraphQLClientOptions`](../interfaces/IotaGraphQLClientOptions.md)\<`Queries`\>

#### Returns

[`IotaGraphQLClient`](IotaGraphQLClient.md)\<`Queries`\>

## Methods

### query()

> **query**\<`Result`, `Variables`\>(`options`): `Promise`\<[`GraphQLQueryResult`](../type-aliases/GraphQLQueryResult.md)\<`Result`\>\>

#### Type Parameters

• **Result** = `Record`\<`string`, `unknown`\>

• **Variables** = `Record`\<`string`, `unknown`\>

#### Parameters

• **options**: [`GraphQLQueryOptions`](../type-aliases/GraphQLQueryOptions.md)\<`Result`, `Variables`\>

#### Returns

`Promise`\<[`GraphQLQueryResult`](../type-aliases/GraphQLQueryResult.md)\<`Result`\>\>

***

### execute()

> **execute**\<`Query`, `Result`, `Variables`\>(`query`, `options`): `Promise`\<[`GraphQLQueryResult`](../type-aliases/GraphQLQueryResult.md)\<`Result`\>\>

#### Type Parameters

• **Query** *extends* `string`

• **Result** = `Queries`\[`Query`\] *extends* [`GraphQLDocument`](../type-aliases/GraphQLDocument.md)\<`R`, `unknown`\> ? `R` : `Record`\<`string`, `unknown`\>

• **Variables** = `Queries`\[`Query`\] *extends* [`GraphQLDocument`](../type-aliases/GraphQLDocument.md)\<`unknown`, `V`\> ? `V` : `Record`\<`string`, `unknown`\>

#### Parameters

• **query**: `Query`

• **options**: `Omit`\<[`GraphQLQueryOptions`](../type-aliases/GraphQLQueryOptions.md)\<`Result`, `Variables`\>, `"query"`\>

#### Returns

`Promise`\<[`GraphQLQueryResult`](../type-aliases/GraphQLQueryResult.md)\<`Result`\>\>
