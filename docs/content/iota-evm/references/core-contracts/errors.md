---
description: 'The errors contract keeps a map of error codes to error message templates. These error codes are used in
request receipts.'
image: /img/logo/WASP_logo_dark.png
tags:
- core-contract
- core-contract-errors
- reference
---

# The `errors` Contract

The `errors` contract is one of the [core contracts](overview.md) on each IOTA Smart Contracts
chain.

The `errors` contract keeps a map of error codes to error message templates.
This allows contracts to store lengthy error strings only once and then reuse them by just providing the error code (and
optional extra values) when producing an error, thus saving storage and gas.

## Entry Points

### `registerError`

Registers an error message template.

#### Parameters

| Name               | Type   | Optional | Description                                                                                                               |
| ------------------ | ------ | -------- | ------------------------------------------------------------------------------------------------------------------------- |
| errorMessageFormat | string | No       | The error message template, which supports standard [go verbs](https://pkg.go.dev/fmt#hdr-Printing) for variable printing |

#### Returns

| Name        | Type                                  | Description                               |
| ----------- | ------------------------------------- | ----------------------------------------- |
| vmErrorCode | [VMErrorCode](./types.md#vmerrorcode) | The error code of the registered template |

---

## Views

### `getErrorMessageFormat`

Returns the message template stored for a given error code.

#### Parameters

| Name        | Type                                  | Optional | Description                               |
| ----------- | ------------------------------------- | -------- | ----------------------------------------- |
| vmErrorCode | [VMErrorCode](./types.md#vmerrorcode) | No       | The error code of the registered template |

#### Returns

| Name               | Type   | Description                |
| ------------------ | ------ | -------------------------- |
| errorMessageFormat | string | The error message template |
