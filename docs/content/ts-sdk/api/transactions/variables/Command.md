# Variable: Command

> `const` **Command**: `EnumSchema`\<`object`\>

## Type declaration

### MoveCall

> **MoveCall**: `ObjectSchema`\<`object`, `undefined`\> = `ProgrammableMoveCall`

#### Type declaration

##### package

> `readonly` **package**: `SchemaWithPipe`\<[`StringSchema`\<`undefined`\>, `TransformAction`\<`string`, `string`\>, `CheckAction`\<`string`, `undefined`\>]\> = `ObjectID`

##### module

> `readonly` **module**: `StringSchema`\<`undefined`\>

##### function

> `readonly` **function**: `StringSchema`\<`undefined`\>

##### typeArguments

> `readonly` **typeArguments**: `ArraySchema`\<`StringSchema`\<`undefined`\>, `undefined`\>

##### arguments

> `readonly` **arguments**: `ArraySchema`\<`GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\>, `undefined`\>

##### \_argumentTypes

> `readonly` **\_argumentTypes**: `OptionalSchema`\<`NullableSchema`\<`ArraySchema`\<`ObjectSchema`\<`object`, `undefined`\>, `undefined`\>, `never`\>, `never`\>

### TransferObjects

> **TransferObjects**: `ObjectSchema`\<`object`, `undefined`\>

#### Type declaration

##### objects

> `readonly` **objects**: `ArraySchema`\<`GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\>, `undefined`\>

##### address

> `readonly` **address**: `GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\> = `Argument`

### SplitCoins

> **SplitCoins**: `ObjectSchema`\<`object`, `undefined`\>

#### Type declaration

##### coin

> `readonly` **coin**: `GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\> = `Argument`

##### amounts

> `readonly` **amounts**: `ArraySchema`\<`GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\>, `undefined`\>

### MergeCoins

> **MergeCoins**: `ObjectSchema`\<`object`, `undefined`\>

#### Type declaration

##### destination

> `readonly` **destination**: `GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\> = `Argument`

##### sources

> `readonly` **sources**: `ArraySchema`\<`GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\>, `undefined`\>

### Publish

> **Publish**: `ObjectSchema`\<`object`, `undefined`\>

#### Type declaration

##### modules

> `readonly` **modules**: `ArraySchema`\<`StringSchema`\<`undefined`\>, `undefined`\>

##### dependencies

> `readonly` **dependencies**: `ArraySchema`\<`SchemaWithPipe`\<[`StringSchema`\<`undefined`\>, `TransformAction`\<`string`, `string`\>, `CheckAction`\<`string`, `undefined`\>]\>, `undefined`\>

### MakeMoveVec

> **MakeMoveVec**: `ObjectSchema`\<`object`, `undefined`\>

#### Type declaration

##### type

> `readonly` **type**: `NullableSchema`\<`StringSchema`\<`undefined`\>, `never`\>

##### elements

> `readonly` **elements**: `ArraySchema`\<`GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\>, `undefined`\>

### Upgrade

> **Upgrade**: `ObjectSchema`\<`object`, `undefined`\>

#### Type declaration

##### modules

> `readonly` **modules**: `ArraySchema`\<`StringSchema`\<`undefined`\>, `undefined`\>

##### dependencies

> `readonly` **dependencies**: `ArraySchema`\<`SchemaWithPipe`\<[`StringSchema`\<`undefined`\>, `TransformAction`\<`string`, `string`\>, `CheckAction`\<`string`, `undefined`\>]\>, `undefined`\>

##### package

> `readonly` **package**: `SchemaWithPipe`\<[`StringSchema`\<`undefined`\>, `TransformAction`\<`string`, `string`\>, `CheckAction`\<`string`, `undefined`\>]\> = `ObjectID`

##### ticket

> `readonly` **ticket**: `GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\> = `Argument`

### $Intent

> **$Intent**: `ObjectSchema`\<`object`, `undefined`\>

#### Type declaration

##### name

> `readonly` **name**: `StringSchema`\<`undefined`\>

##### inputs

> `readonly` **inputs**: `RecordSchema`\<`StringSchema`\<`undefined`\>, `UnionSchema`\<[`GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\>, `ArraySchema`\<`GenericSchema`\<`object` \| `object` \| `object` \| `object`, `object` \| `object` \| `object` \| `object` \| `object`, `BaseIssue`\<`unknown`\>\>, `undefined`\>], `undefined`\>, `undefined`\>

##### data

> `readonly` **data**: `RecordSchema`\<`StringSchema`\<`undefined`\>, `UnknownSchema`, `undefined`\>
