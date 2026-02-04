// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_SYSTEM_STATE_OBJECT_ID } from '@iota/iota-sdk/utils';

export function createPartialTimelockedUnstakeTransaction(
    timelockedStakedObjectId: string,
    unstakeAmount: bigint,
) {
    const tx = new Transaction();
    const splitStakedIota = tx.moveCall({
        target: '0x3::timelocked_staking::split',
        arguments: [tx.object(timelockedStakedObjectId), tx.pure.u64(unstakeAmount)],
    });
    tx.moveCall({
        target: '0x3::timelocked_staking::request_withdraw_stake',
        arguments: [
            tx.sharedObjectRef({
                objectId: IOTA_SYSTEM_STATE_OBJECT_ID,
                initialSharedVersion: 1,
                mutable: true,
            }),
            splitStakedIota,
        ],
    });
    return tx;
}
