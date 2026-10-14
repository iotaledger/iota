// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// A dev inspection creating more than one object has to name them all. Nothing
// it created reached storage, so the names come from the inspection's own
// output.

//# init --addresses test=0x0 --accounts A

//# programmable --sender A --inputs 1 1 @A --dev-inspect
//> SplitCoins(Gas, [Input(0), Input(1)]);
//> TransferObjects([NestedResult(0,0), NestedResult(0,1)], Input(2))
