# Variable: SerializedTransactionDataV2

> `const` **SerializedTransactionDataV2**: `ObjectSchema`\<`object`, `undefined`\>

## Type declaration

### version

> `readonly` **version**: `LiteralSchema`\<`2`, `undefined`\>

### sender

> `readonly` **sender**: `NullishSchema`\<`SchemaWithPipe`\<[`StringSchema`\<`undefined`\>, `TransformAction`\<`string`, `string`\>, `CheckAction`\<`string`, `undefined`\>]\>, `never`\>

### expiration

> `readonly` **expiration**: `NullishSchema`\<`GenericSchema`\<`EnumInputShape`\<`object`\>, `EnumInputShape`\<`object`\>, `BaseIssue`\<`unknown`\>\>, `never`\>

### gasData

> `readonly` **gasData**: `ObjectSchema`\<`object`, `undefined`\> = `GasData`

#### Type declaration

##### budget

> `readonly` **budget**: `NullableSchema`\<`SchemaWithPipe`\<[`UnionSchema`\<[`StringSchema`\<`undefined`\>, `SchemaWithPipe`\<[..., ...]\>], `undefined`\>, `CheckAction`\<`string` \| `number`, `"Invalid u64"`\>]\>, `never`\>

##### price

> `readonly` **price**: `NullableSchema`\<`SchemaWithPipe`\<[`UnionSchema`\<[`StringSchema`\<`undefined`\>, `SchemaWithPipe`\<[..., ...]\>], `undefined`\>, `CheckAction`\<`string` \| `number`, `"Invalid u64"`\>]\>, `never`\>

##### owner

> `readonly` **owner**: `NullableSchema`\<`SchemaWithPipe`\<[`StringSchema`\<`undefined`\>, `TransformAction`\<`string`, `string`\>, `CheckAction`\<`string`, `undefined`\>]\>, `never`\>

##### payment

> `readonly` **payment**: `NullableSchema`\<`ArraySchema`\<`ObjectSchema`\<`object`, `undefined`\>, `undefined`\>, `never`\>

### inputs

> `readonly` **inputs**: `ArraySchema`\<`GenericSchema`\<`EnumInputShape`\<`object`\>, `EnumInputShape`\<`object`\>, `BaseIssue`\<`unknown`\>\>, `undefined`\>

### commands

> `readonly` **commands**: `ArraySchema`\<`GenericSchema`\<`EnumInputShape`\<`object`\>, `EnumInputShape`\<`object`\>, `BaseIssue`\<`unknown`\>\>, `undefined`\>
