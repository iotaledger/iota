---
description: How to interact with the L1 ledger in Solo.
image: /img/logo/WASP_logo_dark.png
tags:
  - how-to
  - evm
  - solo
  - testing
teams:
  - iotaledger/l2-smart-contract
---

# The L1 Ledger

IOTA EVM works as a **layer 2** (**L2**) extension of the _IOTA Move Ledger_, **layer 1** (**L1**).
The address on L1 can own different coins or other [objects](../../../iota-101/objects/object-model.mdx).

In normal operation, the L2 state is maintained by a committee of Wasp _nodes_. The L1 ledger is provided and
maintained by a network of [IOTA](../../../../operator/full-node/overview.mdx) nodes.

The Solo environment runs a local network, simulating the behavior of a real L1 ledger without the
need to run a network of IOTA nodes.

The following example creates a new wallet (private/public key pair) and requests some base tokens from the faucet:

```go
func TestTutorialL1(t *testing.T) {
	env := solo.New(t)
	_, userAddress := env.NewKeyPairWithFunds(env.NewSeedFromIndex(1))
	t.Logf("address of the user is: %s", userAddress)
	numBaseTokens := env.L1BaseTokens(userAddress)
	t.Logf("balance of the user is: %d base tokens", numBaseTokens)
	env.AssertL1BaseTokens(userAddress, iotaclient.FundsFromFaucetAmount)
}
```

The _output_ of the test is:

```log
=== RUN   TestTutorialL1
    wiki_test.go:29: address of the user is: 0x464b5a22a1d5e40ae9f8af129cb81a777c4fae97737754b8fb481d3ed7d84c31
    wiki_test.go:31: balance of the user is: 10000000000 base tokens
--- PASS: TestTutorialL1 (2.28s)
```

The L1 ledger in Solo can be accessed via the Solo instance called `env`.
The ledger is unique for the lifetime of the Solo environment.
Even if several L2 chains are deployed during the test, all of them will live on the same L1 ledger; this way Solo makes it possible to test cross-chain transactions.
(Note that in the test above we did not deploy any chains: the L1 ledger exists independently of any chains.)
