# Type Alias: Command\<Arg\>

> **Command**\<`Arg`\>: `EnumOutputShape`\<`object`\>

## Type declaration

### MoveCall

> **MoveCall**: `object`

### MoveCall.package

> **package**: `string`

### MoveCall.module

> **module**: `string`

### MoveCall.function

> **function**: `string`

### MoveCall.typeArguments

> **typeArguments**: `string`[]

### MoveCall.arguments

> **arguments**: `Arg`[]

### MoveCall.\_argumentTypes?

> `optional` **\_argumentTypes**: [`OpenMoveTypeSignature`](OpenMoveTypeSignature.md)[] \| `null`

### TransferObjects

> **TransferObjects**: `object`

### TransferObjects.objects

> **objects**: `Arg`[]

### TransferObjects.address

> **address**: `Arg`

### SplitCoins

> **SplitCoins**: `object`

### SplitCoins.coin

> **coin**: `Arg`

### SplitCoins.amounts

> **amounts**: `Arg`[]

### MergeCoins

> **MergeCoins**: `object`

### MergeCoins.destination

> **destination**: `Arg`

### MergeCoins.sources

> **sources**: `Arg`[]

### Publish

> **Publish**: `object`

### Publish.modules

> **modules**: `string`[]

### Publish.dependencies

> **dependencies**: `string`[]

### MakeMoveVec

> **MakeMoveVec**: `object`

### MakeMoveVec.type

> **type**: `string` \| `null`

### MakeMoveVec.elements

> **elements**: `Arg`[]

### Upgrade

> **Upgrade**: `object`

### Upgrade.modules

> **modules**: `string`[]

### Upgrade.dependencies

> **dependencies**: `string`[]

### Upgrade.package

> **package**: `string`

### Upgrade.ticket

> **ticket**: `Arg`

### $Intent

> **$Intent**: `object`

### $Intent.name

> **name**: `string`

### $Intent.inputs

> **inputs**: `Record`\<`string`, [`Argument`](Argument.md) \| [`Argument`](Argument.md)[]\>

### $Intent.data

> **data**: `Record`\<`string`, `unknown`\>

## Type Parameters

• **Arg** = [`Argument`](Argument.md)
