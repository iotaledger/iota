# Syntactic rules for mock network tasks in `iota-transactional-test-runner`

Transactional tests simulate network operations through the framework exposed in [iota-transactional-test-runner](https://github.com/iotaledger/iota/tree/develop/crates/iota-transactional-test-runner). The framework is actually built on top of the more generic [move-transactional-test-runner](https://github.com/iotaledger/iota/tree/develop/external-crates/move/crates/move-transactional-test-runner).

This currently used in the following tests:

```
$ cargo tree -i iota-transactional-test-runner
iota-transactional-test-runner v0.1.0 (/home/kodemartin/projects/kinesis/crates/iota-transactional-test-runner)
[dev-dependencies]
├── iota-adapter-transactional-tests v0.1.0 (/home/kodemartin/projects/kinesis/crates/iota-adapter-transactional-tests)
├── iota-graphql-e2e-tests v0.1.0 (/home/kodemartin/projects/kinesis/crates/iota-graphql-e2e-tests)
└── iota-verifier-transactional-tests v0.1.0 (/home/kodemartin/projects/kinesis/crates/iota-verifier-transactional-tests)
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
   - Here, `(x, y)` corresponds to the transaction index and the creation index of the object within that transaction. The placeholder will be replaced with the object ID as a string (like `0xABCD...`).

2. **Named Address Placeholders**
   - **Syntax**: `@{NamedAddr}` or `@{NamedAddr_opt}`
   - Substitutes known accounts and addresses that have been created during the initialization step, e.g. `init --protocol-version 1 --addresses P0=0x0 --accounts A B --simulator`

3. **Cursors**
   - Any string passed to the `--cursors` list will be Base64-encoded and can be accessed in the query as `@{cursor_0}`, `@{cursor_1}`, etc., in the order provided.
   - If the string matches `@{obj_x_y}`, it pairs the object’s ID for (x, y) with the highest checkpoint; if the string matches `@{obj_x_y, checkpoint}`, it pairs the object’s ID with the provided checkpoint. It then BCS-encodes the (objectID, checkpoint) tuple and provides the Base64-encoded cursor value.

All of the above rules (object references, named addresses, raw Base64-encoded strings) can be used in a single query.
Any placeholder that cannot be mapped to a known variable, object, or address will cause an error.

#### Example

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

The above query will replace the placeholder `@{cursor_0}` with the base64-encoded string `{"c":3,"t":1,"tc":1}`.
`@{A}` and `@{P0}` will be replaced with the addresses `A` and `P0` respectively that were created during the initialization step.

### `force-object-snapshot-catchup`

### `bench`

### `init`

### `print-bytecode`

> Translates the given Move IR module into bytecode, then prints a textual
> representation of that bytecode

### `publish`

### `run`
