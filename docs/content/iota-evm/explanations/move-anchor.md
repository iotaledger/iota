---
description: The IOTA EVM can be interacted with using the ISC Move packages.
teams:
 - iotaledger/l2-smart-contract
---

# ISC Move Packages

As mentioned in the [state anchoring](./states.md) section, the IOTA EVM anchors into Move using the Anchor object. The Anchor is part of the ISC Move packages, a set of smart contracts that provide the necessary functionality for the two VMs to interact.

## anchor

The `anchor` package defines the Anchor object, which manages all L1 funds of the chain. It provides functions to create new chains, update their state, and receive requests.

## assets_bag

The `assets_bag` package defines the `AssetsBag` object used to manage assets in the ISC ecosystem. It is, for example, used to send funds to the EVM by interacting with the `request` package.

## request

The `request` package defines the logic of interacting with the EVM through [core contracts](./core-contracts.md). It provides functions to create requests that contain the BCS-encoded parameter to call a core contract.
