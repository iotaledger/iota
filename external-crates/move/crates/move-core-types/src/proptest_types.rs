// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use proptest::{collection::vec, prelude::*};

use crate::{account_address::AccountAddress, transaction_argument::TransactionArgument};

impl Arbitrary for TransactionArgument {
    type Parameters = ();
    fn arbitrary_with(_args: ()) -> Self::Strategy {
        prop_oneof![
            any::<bool>().prop_map(TransactionArgument::Bool),
            any::<u64>().prop_map(TransactionArgument::U64),
            any::<AccountAddress>().prop_map(TransactionArgument::Address),
            vec(any::<u8>(), 0..10).prop_map(TransactionArgument::U8Vector),
        ]
        .boxed()
    }

    type Strategy = BoxedStrategy<Self>;
}
