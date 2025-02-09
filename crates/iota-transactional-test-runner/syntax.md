# Syntactic rules for mock network tasks in `iota-transactional-test-runner`

Transactional tests simulate network operations through the framework exposed in [iota-transactional-test-runner](https://github.com/iotaledger/iota/tree/develop/crates/iota-transactional-test-runner). The framework is actually built on top of the more generic [move-transactional-test-runner](https://github.com/iotaledger/iota/tree/develop/external-crates/move/crates/move-transactional-test-runner).

This currently used in the following tests:

```
$ cargo tree -i iota-transactional-test-runner
iota-transactional-test-runner v0.1.0 (crates/iota-transactional-test-runner)
[dev-dependencies]
├── iota-adapter-transactional-tests v0.1.0 (crates/iota-adapter-transactional-tests)
├── iota-graphql-e2e-tests v0.1.0 (crates/iota-graphql-e2e-tests)
└── iota-verifier-transactional-tests v0.1.0 (crates/iota-verifier-transactional-tests)
```

## Common rules

The framework introduces an ad-hoc syntax for defining network related operations/tasks as an extension to `move/mvir` files.

The syntax uses comments with the `//#` prefix to begin blocks of continuous non-empty lines that are eventually used to parse the underlying tasks and any additional `data`. Empty lines define the boundaries of each block. So the basic syntax for all tasks is the following:

```
<empty-line>
//# <task> [OPTIONS]
[<task-data>]
...
<empty-line>
```

For example:

```
                                                                        [empty-line]
//# run-graphql --show-usage --show-headers --show-service-version      [task]
{                                                                       [data]
  checkpoint {                                                          [data]
    sequenceNumber                                                      [data]
  }                                                                     [data]
}                                                                       [data]
                                                                        [empty-line]
```

The syntax rules for the `data` are specific to each task and will be discussed
in the respective sections.

## Supported tasks

### `view-object`

### `transfer-object`

### `consensus-commit-prologue`

### `programmable`

### `upgrade`

### `stage-package`

### `set-address`

### `create-checkpoint`

### `advance-epoch`

### `advance-clock`

### `set-random-state`

### `view-checkpoint`

### `run-graphql`

Allows to execute GraphQL queries with optional options and returns the output.

#### Syntax:

```
//# run-graphql [OPTIONS]
<GraphQL-query>
```

#### Options:

```
--show-usage: Displays usage information for the command.
--show-headers: Includes HTTP headers in the output.
--show-service-version: Displays the version of the service handling the GraphQL query.
--cursors <cursor-list>: Specifies a list of cursors to be used within the query.
```

#### Query Interpolation

The command supports **query interpolation**, allowing you to dynamically replace parts of the GraphQL query at runtime.
It supports the following placeholders:

1. **Object Placeholders**
   - **Syntax**: `@{obj_x_y}` or `@{obj_x_y_opt}`
   - Here, `(x, y)` corresponds to the task index and the creation index of the object within that task. The placeholder will be replaced with the object ID as a string (like `0xABCD...`).

2. **Named Address Placeholders**
   - **Syntax**: `@{NamedAddr}` or `@{NamedAddr_opt}`
   - Substitutes known accounts and addresses that have been created during the initialization step, e.g. `init --protocol-version 1 --addresses P0=0x0 --accounts A B --simulator`

3. **Cursors**
   - **Syntax**: `//# run-graphql --cursors string1 string2 ...`
     - Depending on the query, the raw strings passed to `--cursors` might be required in JSON, BCS or any other format that the query expects.
     - Each string passed is automatically Base64-encoded (as all cursor values are expected to be Base64-encoded) and can be accessed in the query as `@{cursor_0}`, `@{cursor_1}`, etc., in the order provided.
     - To generate cursor values from objects at runtime, the strings passed must correspond to the format `@{obj_x_y}` or `@{obj_x_y, checkpoint}` and are translated to Base64-encoded object cursors.

All of the above rules (object placeholders, named address placeholders, cursor strings) can be used in a single query.
Any placeholder or cursor that cannot be mapped to a known variable, object, or address will cause an error.

#### Examples

The following example query will replace the placeholder `@{cursor_0}` with the Base64-encoded [transaction block cursor](../../crates/iota-graphql-rpc/src/types/transaction_block.rs) `{"c":3,"t":1,"tc":1}` where `c` is the checkpoint sequence number, `t` is the transaction sequence number, and `tc` is the transaction checkpoint number.
Cursor values depend on the query and the underlying schema. The cursor value above is specific to the GraphQL `transactionBlocks` query.
`@{A}` and `@{P0}` will be replaced with the addresses `A` and `P0` respectively that were created during the initialization step.

```
//# run-graphql --cursors {"c":3,"t":1,"tc":1}
{
  transactionBlocks(first: 1, after: "@{cursor_0}", filter: {signAddress: "@{A}"}) {
    nodes {
      sender {
        fakeCoinBalance: balance(type: "@{P0}::fake::FAKE") {
          totalBalance
        }
        allBalances: balances {
          nodes {
            coinType {
              repr
            }
            coinObjectCount
            totalBalance
          }
        }
      }
    }
  }
}
```

An example of a query that generates an object cursor at runtime:

```
//# run-graphql --cursors @{obj_6_0}
{
  address(address: "@{A}") {
    objects(first: 2 after: "@{cursor_0}") {
      edges {
        node {
          contents {
            json
          }
        }
      }
    }
  }
}
```

### `force-object-snapshot-catchup`

### `bench`

### `init`

### `print-bytecode`

> Translates the given Move IR module into bytecode, then prints a textual
> representation of that bytecode

### `publish`

### `run`

### Common rules

Object identifiers (ObjectID) follow specific conventions that allow referencing objects across different test commands. This section describes how object IDs work, including how they are used in subcommands and programmable transactions (PTBs).

#### Understanding Object Identifiers (object(x,y))

Object identifiers in test files typically take the form:

```
object(x,y)
```

- x: Represents the task number in which the object was created.
- y: Represents the index of the object within that task.

For instance, object(1,0) means:

The object was created in task 1.
It was the first object (0-based index) created within that task.
In `.move` test files, object references are often written as:

```move
//# view-object 1,0
```

Here, 1,0 refers to the object created in task 1, index 0.

#### Versioned Object Identifiers (object(x,y)@version)

Object references in PTBs can include a version number.

Example:

```
//# programmable --sender A --inputs object(1,0)@2 @acc1
//> TransferObjects([Input(0)], Input(1))
```

Here:

- @2: Indicates version 2 of the object.

Why specify versions?

- Objects mutate over time, especially in transactions.
- If an object is referenced in different states, the version ensures the correct state is used.
- If omitted, the latest known version of the object is used.

#### Usage example

`.move` file example:

```move
//# init --addresses P0=0x0 --accounts A --protocol-version 1 --simulator

//# programmable --sender A --inputs 1000 @A
//> SplitCoins(Gas, [Input(0)]);
//> TransferObjects([Result(0)], Input(1))

//# view-object 1,0

//# create-checkpoint

//# programmable --sender A --inputs object(1,0)@2
//> MergeCoins(Gas, [Input(0)])
```

`.exp` file example:

```
processed 5 tasks

init:
A: object(0,0)

task 1 'programmable'. lines 3-5:
created: object(1,0)
mutated: object(0,0)
gas summary: computation_cost: 1000000, storage_cost: 1976000,  storage_rebate: 0, non_refundable_storage_fee: 0

task 2 'view-object'. lines 7-7:
Owner: Account Address ( A )
Version: 2
Contents: iota::coin::Coin<iota::iota::IOTA> {id: iota::object::UID {id: iota::object::ID {bytes: fake(1,0)}}, balance: iota::balance::Balance<iota::iota::IOTA> {value: 1000u64}}

task 3 'create-checkpoint'. lines 9-9:
Checkpoint created: 1

task 4 'programmable'. lines 11-12:
mutated: object(0,0)
deleted: object(1,0)
gas summary: computation_cost: 1000000, storage_cost: 988000,  storage_rebate: 1976000, non_refundable_storage_fee: 0
```

Explanation:

In task 1:

- Object object(1,0) is created.

In task 4:

- object(0,0) is mutated.
- object(1,0) is deleted after being merged into Gas general object.

Summary:

1. object(x,y): References an object created in task x, at index y.
2. view-object x,y: Displays the current state of the object.
3. object(x,y)@version: Specifies a particular version of the object.
4. Objects in PTBs: Used for transfers (TransferObjects), merging (MergeCoins), and execution of transactions.
