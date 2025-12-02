// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authentication using mutable abstract account

//# init --addresses test=0x0 --accounts A --default-aa

//# publish --sender A --dependencies aa
module test::authenticate;

use aa::abstract_account::AbstractAccount;
use iota::auth_context::AuthContext;

public fun authenticate(_account: &AbstractAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# init-abstract-acc --sender A test authenticate authenticate

//# view-object 2,0

//# abstract --account object(2,0) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
