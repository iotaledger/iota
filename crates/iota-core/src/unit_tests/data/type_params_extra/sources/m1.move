// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module type_params::m1 {
    use iota::transfer;

    public entry fun transfer_object<T: key + store>(o: T, recipient: address) {
        transfer::public_transfer(o, recipient);
    }


}
