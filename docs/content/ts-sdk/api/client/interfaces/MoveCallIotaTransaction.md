# Interface: MoveCallIotaTransaction

The transaction for calling a Move function, either an entry function or a public function (which
cannot return references).

## Properties

### arguments?

> `optional` **arguments**: [`IotaArgument`](../type-aliases/IotaArgument.md)[]

The arguments to the function.

---

### function

> **function**: `string`

The function to be called.

---

### module

> **module**: `string`

The specific module in the package containing the function.

---

### package

> **package**: `string`

The package containing the module and function.

---

### type\_arguments?

> `optional` **type\_arguments**: `string`[]

The type arguments to the function.
