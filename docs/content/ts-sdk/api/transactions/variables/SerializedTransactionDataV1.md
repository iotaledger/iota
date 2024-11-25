# Variable: SerializedTransactionDataV1

> `const` **SerializedTransactionDataV1**: `ObjectSchema`\<`object`, `undefined`\>

## Type declaration

### version

> `readonly` **version**: `LiteralSchema`\<`1`, `undefined`\>

### sender

> `readonly` **sender**: `OptionalSchema`\<`StringSchema`\<`undefined`\>, `never`\>

### expiration

> `readonly` **expiration**: `NullishSchema`\<`UnionSchema`\<[`ObjectSchema`\<`object`, `undefined`\>, `ObjectSchema`\<`object`, `undefined`\>], `undefined`\>, `never`\>

### gasConfig

> `readonly` **gasConfig**: `ObjectSchema`\<`object`, `undefined`\> = `GasConfig`

#### Type declaration

##### budget

> `readonly` **budget**: `OptionalSchema`\<`SchemaWithPipe`\<[`UnionSchema`\<[`NumberSchema`\<`undefined`\>, `StringSchema`\<`undefined`\>, `BigintSchema`\<`undefined`\>], `undefined`\>, `CheckAction`\<`string` \| `number` \| `bigint`, `undefined`\>]\>, `never`\>

##### price

> `readonly` **price**: `OptionalSchema`\<`SchemaWithPipe`\<[`UnionSchema`\<[`NumberSchema`\<`undefined`\>, `StringSchema`\<`undefined`\>, `BigintSchema`\<`undefined`\>], `undefined`\>, `CheckAction`\<`string` \| `number` \| `bigint`, `undefined`\>]\>, `never`\>

##### payment

> `readonly` **payment**: `OptionalSchema`\<`ArraySchema`\<`ObjectSchema`\<`object`, `undefined`\>, `undefined`\>, `never`\>

##### owner

> `readonly` **owner**: `OptionalSchema`\<`StringSchema`\<`undefined`\>, `never`\>

### inputs

> `readonly` **inputs**: `ArraySchema`\<`UnionSchema`\<[`ObjectSchema`\<`object`, `undefined`\>, `ObjectSchema`\<`object`, `undefined`\>], `undefined`\>, `undefined`\>

### transactions

> `readonly` **transactions**: `ArraySchema`\<`UnionSchema`\<[`ObjectSchema`\<`object`, `undefined`\>, `ObjectSchema`\<`object`, `undefined`\>, `ObjectSchema`\<`object`, `undefined`\>], `undefined`\>, `undefined`\>
