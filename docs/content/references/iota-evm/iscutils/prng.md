---
title: prng
description: 'Not recommended for generating cryptographic secure randomness'
---

## PRNG

This library is used to generate pseudorandom numbers

_Not recommended for generating cryptographic secure randomness_

### PRNGState

_Represents the state of the PRNG_

```solidity
struct PRNGState {
  bytes32 state;
}
```

### generateRandomHash

```solidity
function generateRandomHash(struct PRNG.PRNGState self) internal returns (bytes32)
```

Generate a new pseudorandom hash

_Takes the current state, hashes it and returns the new state._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| self | struct PRNG.PRNGState | The PRNGState struct to use and alter the state |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bytes32 | The generated pseudorandom hash |

### generateRandomNumber

```solidity
function generateRandomNumber(struct PRNG.PRNGState self) internal returns (uint256)
```

Generate a new pseudorandom number

_Takes the current state, hashes it and returns the new state._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| self | struct PRNG.PRNGState | The PRNGState struct to use and alter the state |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint256 | The generated pseudorandom number |

### generateRandomNumberInRange

```solidity
function generateRandomNumberInRange(struct PRNG.PRNGState self, uint256 min, uint256 max) internal returns (uint256)
```

Generate a new pseudorandom number in a given range [min, max)

_Takes the current state, hashes it and returns the new state. It constrains the returned number to the bounds of min (inclusive) and max (exclusive)._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| self | struct PRNG.PRNGState | The PRNGState struct to use and alter the state |
| min | uint256 |  |
| max | uint256 |  |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint256 | The generated pseudorandom number constrained to the bounds of [min, max) |

### seed

```solidity
function seed(struct PRNG.PRNGState self, bytes32 entropy) internal
```

Seed the PRNG

_The seed should not be zero_

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| self | struct PRNG.PRNGState | The PRNGState struct to update the state |
| entropy | bytes32 | The seed value (entropy) |

