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

### `force-object-snapshot-catchup`

### `bench`

### `init`

### `print-bytecode`

> Translates the given Move IR module into bytecode, then prints a textual
> representation of that bytecode

### `publish`

### `run`
