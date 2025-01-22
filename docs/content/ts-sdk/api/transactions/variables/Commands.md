# Variable: Commands

> `const` **Commands**: `object`

Simple helpers used to construct transactions:

## Type declaration

### MoveCall()

#### Parameters

• **input**: `object` \| `object`

#### Returns

`TransactionShape`\<`"MoveCall"`\>

### TransferObjects()

#### Parameters

• **objects**: (`object` \| `object` \| `object` \| `object`)[]

• **address**: `object` \| `object` \| `object` \| `object`

#### Returns

`TransactionShape`\<`"TransferObjects"`\>

### SplitCoins()

#### Parameters

• **coin**: `object` \| `object` \| `object` \| `object`

• **amounts**: (`object` \| `object` \| `object` \| `object`)[]

#### Returns

`TransactionShape`\<`"SplitCoins"`\>

### MergeCoins()

#### Parameters

• **destination**: `object` \| `object` \| `object` \| `object`

• **sources**: (`object` \| `object` \| `object` \| `object`)[]

#### Returns

`TransactionShape`\<`"MergeCoins"`\>

### Publish()

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.modules**: `string`[] \| `number`[][]

• **\_\_namedParameters.dependencies**: `string`[]

#### Returns

`TransactionShape`\<`"Publish"`\>

### Upgrade()

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.modules**: `string`[] \| `number`[][]

• **\_\_namedParameters.dependencies**: `string`[]

• **\_\_namedParameters.package**: `string`

• **\_\_namedParameters.ticket**: `object` \| `object` \| `object` \| `object`

#### Returns

`TransactionShape`\<`"Upgrade"`\>

### MakeMoveVec()

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.type?**: `string`

• **\_\_namedParameters.elements**: (`object` \| `object` \| `object` \| `object`)[]

#### Returns

`TransactionShape`\<`"MakeMoveVec"`\>

### Intent()

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.name**: `string`

• **\_\_namedParameters.inputs?**: `Record`\<`string`, `object` \| `object` \| `object` \| `object` \| (`object` \| `object` \| `object` \| `object`)[]\> = `{}`

• **\_\_namedParameters.data?**: `Record`\<`string`, `unknown`\> = `{}`

#### Returns

`TransactionShape`\<`"$Intent"`\>
