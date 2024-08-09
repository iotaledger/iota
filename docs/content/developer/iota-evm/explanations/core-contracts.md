---
description: There currently are 6 core smart contracts that are always deployed on each  chain, root, _default, accounts, blob, blocklog, and governance.
image: /img/banner/banner_wasp_core_contracts_overview.png
tags:
  - core-contract
  - reference
  - evm
---

# Core Contracts

![Wasp Node Core Contracts Overview](/img/banner/banner_wasp_core_contracts_overview.png)

There are currently 7 core smart contracts that are always deployed on each
chain. These are responsible for the vital functions of the chain and
provide infrastructure for all other smart contracts:

- [`root`](../../../references/iota-evm/core-contracts/root.md): Responsible for the initialization of the chain, maintains registry of deployed contracts.

- [`accounts`](../../../references/iota-evm/core-contracts/accounts.md): Manages the on-chain ledger of accounts.

- [`blob`](../../../references/iota-evm/core-contracts/blob.md): Responsible for the registry of binary objects of arbitrary size.

- [`blocklog`](../../../references/iota-evm/core-contracts/blocklog.md): Keeps track of the blocks and receipts of requests that were processed by the chain.

- [`governance`](../../../references/iota-evm/core-contracts/governance.md): Handles the administrative functions of the chain. For example: rotation of the committee of validators of the chain, fees and other chain-specific configurations.

- [`errors`](../../../references/iota-evm/core-contracts/errors.md): Keeps a map of error codes to error messages templates. These error codes are used in request receipts.

- [`evm`](../../../references/iota-evm/core-contracts/evm.md): Provides the necessary infrastructure to accept Ethereum
  transactions and execute EVM code.
