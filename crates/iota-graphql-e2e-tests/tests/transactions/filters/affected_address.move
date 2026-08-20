// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Tests the `affectedAddress` filter: a transaction affects an address when
// the address is the sender or a recipient. Also tests the AFFECTED relation
// on `Address.transactionBlocks` and combining the filter with other filters.

//# init --protocol-version 15 --addresses Test=0x0 --accounts A B C --simulator

//# programmable --sender A --inputs 1000000 @B
//> SplitCoins(Gas, [Input(0)]);
//> TransferObjects([Result(0)], Input(1))

//# programmable --sender B --inputs 2000000 @C
//> SplitCoins(Gas, [Input(0)]);
//> TransferObjects([Result(0)], Input(1))

//# programmable --sender B --inputs 3000000 @B
//> SplitCoins(Gas, [Input(0)]);
//> TransferObjects([Result(0)], Input(1))

//# create-checkpoint

//# run-graphql
query {
  affectedAsSender: transactionBlocks(filter: { affectedAddress: "@{A}" }) {
    nodes { digest }
  }
  affectedAsRecipient: transactionBlocks(filter: { affectedAddress: "@{C}" }) {
    nodes { digest }
  }
  affectedBoth: transactionBlocks(filter: { affectedAddress: "@{B}" }) {
    nodes { digest }
  }
  combinedWithKind: transactionBlocks(filter: { affectedAddress: "@{B}", kind: PROGRAMMABLE_TX }, scanLimit: 10) {
    nodes { digest }
  }
  combinedWithSent: transactionBlocks(filter: { affectedAddress: "@{B}", sentAddress: "@{B}" }) {
    nodes { digest }
  }
  combinedWithOtherSent: transactionBlocks(filter: { affectedAddress: "@{C}", sentAddress: "@{B}" }) {
    nodes { digest }
  }
  affectedViaAddress: address(address: "@{B}") {
    transactionBlocks(relation: AFFECTED) {
      nodes { digest }
    }
  }
}
