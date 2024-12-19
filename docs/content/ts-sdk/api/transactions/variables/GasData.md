# Variable: GasData

> `const` **GasData**: `ObjectSchema`\<`object`, `undefined`\>

## Type declaration

### budget

> `readonly` **budget**: `NullableSchema`\<`SchemaWithPipe`\<[`UnionSchema`\<[`StringSchema`\<`undefined`\>, `SchemaWithPipe`\<[`NumberSchema`\<`undefined`\>, `IntegerAction`\<`number`, `undefined`\>]\>], `undefined`\>, `CheckAction`\<`string` \| `number`, `"Invalid u64"`\>]\>, `never`\>

### price

> `readonly` **price**: `NullableSchema`\<`SchemaWithPipe`\<[`UnionSchema`\<[`StringSchema`\<`undefined`\>, `SchemaWithPipe`\<[`NumberSchema`\<`undefined`\>, `IntegerAction`\<`number`, `undefined`\>]\>], `undefined`\>, `CheckAction`\<`string` \| `number`, `"Invalid u64"`\>]\>, `never`\>

### owner

> `readonly` **owner**: `NullableSchema`\<`SchemaWithPipe`\<[`StringSchema`\<`undefined`\>, `TransformAction`\<`string`, `string`\>, `CheckAction`\<`string`, `undefined`\>]\>, `never`\>

### payment

> `readonly` **payment**: `NullableSchema`\<`ArraySchema`\<`ObjectSchema`\<`object`, `undefined`\>, `undefined`\>, `never`\>
