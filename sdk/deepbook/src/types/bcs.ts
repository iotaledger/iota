// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bcs } from '@iota/iota-sdk/bcs';

export const BcsOrder = bcs.struct('Order', {
    orderId: bcs.u64(),
    clientOrderId: bcs.u64(),
    price: bcs.u64(),
    originalQuantity: bcs.u64(),
    quantity: bcs.u64(),
    isBid: bcs.bool(),
    owner: bcs.Address,
    expireTimestamp: bcs.u64(),
    selfMatchingPrevention: bcs.u8(),
});
