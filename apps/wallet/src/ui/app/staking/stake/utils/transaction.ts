// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_SYSTEM_STATE_OBJECT_ID } from '@iota/iota-sdk/utils';

export function createStakeTransaction(amount: bigint, validator: string) {
    const tx = new Transaction();
    const stakeCoin = tx.splitCoins(tx.gas, [amount]);
    tx.moveCall({
        target: '0x3::iota_system::request_add_stake',
        arguments: [
            tx.sharedObjectRef({
                objectId: IOTA_SYSTEM_STATE_OBJECT_ID,
                initialSharedVersion: 1,
                mutable: true,
            }),
            stakeCoin,
            tx.pure.address(validator),
        ],
    });
    return tx;
}

export function createUnstakeTransaction(stakedIotaId: string) {
    const tx = new Transaction();
    tx.moveCall({
        target: '0x3::iota_system::request_withdraw_stake',
        arguments: [tx.object(IOTA_SYSTEM_STATE_OBJECT_ID), tx.object(stakedIotaId)],
    });
    return tx;
}
