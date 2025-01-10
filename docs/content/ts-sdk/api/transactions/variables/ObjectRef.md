# Variable: ObjectRef

> `const` **ObjectRef**: `ObjectSchema`\<`object`, `undefined`\>

## Type declaration

### objectId

> `readonly` **objectId**: `SchemaWithPipe`\<[`StringSchema`\<`undefined`\>, `TransformAction`\<`string`, `string`\>, `CheckAction`\<`string`, `undefined`\>]\> = `IotaAddress`

### version

> `readonly` **version**: `SchemaWithPipe`\<[`UnionSchema`\<[`StringSchema`\<`undefined`\>, `SchemaWithPipe`\<[`NumberSchema`\<`undefined`\>, `IntegerAction`\<`number`, `undefined`\>]\>], `undefined`\>, `CheckAction`\<`string` \| `number`, `"Invalid u64"`\>]\> = `JsonU64`

### digest

> `readonly` **digest**: `StringSchema`\<`undefined`\>
