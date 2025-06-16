// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module 0x1::t {

fun f() {
    transfer::public_transfer(old_phone, @examples);
}
}
