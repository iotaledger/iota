---
description: Example of a _Solo_ test. It deploys a new chain and invokes some view calls.
image: /img/logo/WASP_logo_dark.png
tags:
  - how-to
  - evm
  - solo
  - testing
teams:
  - iotaledger/l2-smart-contract
---

# First Example

The following is an example of a _Solo_ test. It deploys a new chain and invokes some view calls in the
[`root`](../../references/core-contracts/root.md) and [`governance`](../../references/core-contracts/governance.md)
[core contracts](../../references/core-contracts/overview.md).

:::info L1 Network

To run the tests a local IOTA network is required. That is what

```go
func TestMain(m *testing.M) {
    l1starter.TestMain(m)
}
```

is for. You will need to call it once before running any tests that use the Solo framework.

:::

```go
package solo_test

import (
    "testing"
    "github.com/iotaledger/wasp/packages/solo"
    "github.com/iotaledger/wasp/packages/vm/core/corecontracts"
    "github.com/stretchr/testify/require"
    "github.com/iotaledger/wasp/packages/testutil/l1starter"
)

func TestMain(m *testing.M) {
    l1starter.TestMain(m)
}

func TestTutorialFirst(t *testing.T) {
    env := solo.New(t)
    chain := env.NewChain()
    // calls views governance::ViewGetChainInfo and root:: ViewGetContractRecords
    chainID, chainOwnerID, coreContracts := chain.GetInfo()
    // assert that all core contracts are deployed
    require.GreaterOrEqual(t, len(coreContracts), len(corecontracts.All))
    
    t.Logf("chain ID: %s", chainID.String())
    t.Logf("chain owner ID: %s", chainOwnerID.String())
    for hname, rec := range coreContracts {
        t.Logf("    Core contract %q: %s", rec.Name, hname)
    }
}
```

The output of the test will be something like this:

```log
=== RUN   TestTutorialFirst
config file .testconfig not found - using default values
02:54.102456000 INFO    TestTutorialFirst       WaitForNextVersionForTesting: Found the updated version of obj{id=0x0316f37b5651ca53efd28835209b5054636b9ae40c602db9f442eeb3d2843509, version=2, digest=8zFknGz5Jn71mBfRgGpGVN9DG6i5RWvG5cUi69PutcZY}, which is: {0x0316f37b5651ca53efd28835209b5054636b9ae40c602db9f442eeb3d2843509 3 BgPrbjBJKae1ZTLPxyktpJ3reJXPqR5zQegDgARuSTDw}
02:54.102485000 INFO    TestTutorialFirst       Chain Originator address: &{0x140000d2e10 0x140000d2e40}
02:54.102504000 INFO    TestTutorialFirst       GAS COIN BEFORE PULL: obj{id=0x24b93a63294dd2d601787bc2c6a0c3c6cdc8bbff5a256fcd9bf0b44692b6b2c3, version=3, digest=6sfviRVcVqTqZkigr2PqUbP62aG6a6KS5TbaSyZ7TB9A}
02:54.102595000 INFO    TestTutorialFirst.L1ParamsFetcher       Fetching latest L1Params...
02:54.442070000 INFO    TestTutorialFirst                       WaitForNextVersionForTesting: Found the updated version of obj{id=0x0316f37b5651ca53efd28835209b5054636b9ae40c602db9f442eeb3d2843509, version=3, digest=BgPrbjBJKae1ZTLPxyktpJ3reJXPqR5zQegDgARuSTDw}, which is: {0x0316f37b5651ca53efd28835209b5054636b9ae40c602db9f442eeb3d2843509 4 4cg6HjxVSUsHfaZCgT2dHVr3DmUcqBg5BfKtXtNdFF94}
02:54.442118000 INFO    TestTutorialFirst                       deployed chain 'chain1' - ID: 0x0e3ad1d73c790603e4239e9db4d1c0f79cf6e69956c89bee7926ebc6b8fa6aa5 - anchor owner: 0x67fc46395ed92449249dc61336ad51eef1b6501cec11f26f2e305e458d576c65 - chain admin: 0xf752b522fd4ef6105d2d15a21956327bb7492258d4276fd71e78060282460a42 - origin trie root: 476f6131eedd2379f8cd5cd363cf48fc0416a3ed
02:54.442140000 INFO    TestTutorialFirst.chain1                chain 'chain1' deployed. Chain ID: 0x0e3ad1d73c790603e4239e9db4d1c0f79cf6e69956c89bee7926ebc6b8fa6aa5
02:54.895435000 INFO    TestTutorialFirst                       WaitForNextVersionForTesting: Found the updated version of obj{id=0x24b93a63294dd2d601787bc2c6a0c3c6cdc8bbff5a256fcd9bf0b44692b6b2c3, version=3, digest=6sfviRVcVqTqZkigr2PqUbP62aG6a6KS5TbaSyZ7TB9A}, which is: {0x24b93a63294dd2d601787bc2c6a0c3c6cdc8bbff5a256fcd9bf0b44692b6b2c3 5 DQokmMDn38zEZifMHsVGd6GbsJ8xxUWadYyzyYPhcrak}
02:54.896287000 INFO    TestTutorialFirst                       solo publisher: new_block 0x0e3ad1d73c790603e4239e9db4d1c0f79cf6e69956c89bee7926ebc6b8fa6aa5 0x0e3ad1d73c790603e4239e9db4d1c0f79cf6e69956c89bee7926ebc6b8fa6aa5 | - (new_block)
02:54.896298000 INFO    TestTutorialFirst.chain1                state transition --> #1. Requests in the block: 1
02:54.896435000 INFO    TestTutorialFirst                       solo publisher: receipt 0x0e3ad1d73c790603e4239e9db4d1c0f79cf6e69956c89bee7926ebc6b8fa6aa5 0x0e3ad1d73c790603e4239e9db4d1c0f79cf6e69956c89bee7926ebc6b8fa6aa5 | 0xf752b522fd4ef6105d2d15a21956327bb7492258d4276fd71e78060282460a42 (receipt)
02:54.896481000 INFO    TestTutorialFirst                       solo publisher: block_events 0x0e3ad1d73c790603e4239e9db4d1c0f79cf6e69956c89bee7926ebc6b8fa6aa5 0x0e3ad1d73c790603e4239e9db4d1c0f79cf6e69956c89bee7926ebc6b8fa6aa5 | - (block_events)
02:54.897057000 INFO    TestTutorialFirst.chain1                REQ: 'tx/0xa47f42f8d7c4c924b04ebc54e56f028a21d3a16049df2f888ff2e1b35fa98b0b'
    tutorial_test.go:30: chain ID: 0x0e3ad1d73c790603e4239e9db4d1c0f79cf6e69956c89bee7926ebc6b8fa6aa5
    tutorial_test.go:31: chain owner ID: 0xf752b522fd4ef6105d2d15a21956327bb7492258d4276fd71e78060282460a42
    tutorial_test.go:33:     Core contract "root": 0xcebf5908
    tutorial_test.go:33:     Core contract "governance": 0x17cf909f
    tutorial_test.go:33:     Core contract "testcore": 0x370d33ad
    tutorial_test.go:33:     Core contract "errors": 0x8f3a8bb3
    tutorial_test.go:33:     Core contract "evm": 0x07cb02c1
    tutorial_test.go:33:     Core contract "testerrors": 0x6cb85de2
    tutorial_test.go:33:     Core contract "accounts": 0x3c4b5e02
    tutorial_test.go:33:     Core contract "blocklog": 0xf538ef2b
    tutorial_test.go:33:     Core contract "ManyEventsContract": 0x19cdb859
    tutorial_test.go:33:     Core contract "inccounter": 0xaf2438e9
--- PASS: TestTutorialFirst (5.70s)
```

:::note

- The example uses [`stretchr/testify`](https://github.com/stretchr/testify) for assertions, but it is not strictly
  required.
- Addresses, chain IDs and other hashes should be the same on each run of the test because Solo uses a constant seed by
  default.
- The timestamps shown in the log come from the computer's timer, but the Solo environment operates on its own logical
  time.

:::

The [core contracts](../../references/core-contracts/overview.md) listed in the log are automatically deployed on each
new chain. The log also shows their _contract IDs_.

The output fragment in the log `state transition --> #1` means that the state of the chain has changed from block index
0 (the origin index of the empty state) to block index 1. State #0 is the empty origin state, and #1 always contains all
core smart contracts deployed on the chain, as well as other data internal to the chain itself, such as the _chainID_
and the _chain owner ID_.

The _chain ID_ and _chain owner ID_ are, respectively, the ID of the deployed chain, and the address of the L1 account
that triggered the deployment of the chain (which is automatically generated by Solo in our example, but it can be
overridden when calling `env.NewChain`).
