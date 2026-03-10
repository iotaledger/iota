// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bcs } from '@iota/iota-sdk/bcs';

export const DummyFieldBcs = bcs.struct('DummyFieldObj', {
    dummy_field: bcs.bool(),
});

export const NameBcs = bcs.struct('Name', {
    labels: bcs.vector(bcs.string()),
});

const RangeBcs = bcs.struct('Range', {
    from: bcs.u8(),
    to: bcs.u8(),
});

const CouponRulesBcs = bcs.struct('CouponRules', {
    length: bcs.option(RangeBcs),
    available_claims: bcs.option(bcs.u64()),
    user: bcs.option(bcs.Address),
    expiration: bcs.option(bcs.u64()),
    years: bcs.option(RangeBcs),
    can_stack: bcs.bool(),
});

export const CouponHouseBcs = bcs.struct('CouponHouse', {
    coupons: bcs.struct('Coupons', {
        coupons: bcs.struct('Table', {
            id: bcs.struct('UID', {
                id: bcs.struct('ID', {
                    bytes: bcs.Address,
                }),
            }),
            size: bcs.u64(),
        }),
    }),
    version: bcs.u8(),
    id: bcs.struct('UID', {
        id: bcs.struct('ID', {
            bytes: bcs.Address,
        }),
    }),
});

export const CouponBcs = bcs.struct('Coupon', {
    kind: bcs.u8(),
    amount: bcs.u64(),
    rules: CouponRulesBcs,
});
