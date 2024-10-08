// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionBlock } from '@iota/iota-sdk/transactions';
import { IOTA_SYSTEM_STATE_OBJECT_ID } from '@iota/iota-sdk/utils';

export function createTimelockedUnstakeTransaction(timelockedStakedObjectIds: string[]) {
    const tx = new TransactionBlock();
    // TODO: check the max tx limit per ptb
    for (const timelockedStakedObjectId of timelockedStakedObjectIds) {
        tx.moveCall({
            target: `0x3::timelocked_staking::request_withdraw_stake`,
            arguments: [
                tx.sharedObjectRef({
                    objectId: IOTA_SYSTEM_STATE_OBJECT_ID,
                    initialSharedVersion: 1,
                    mutable: true,
                }),
                tx.object(timelockedStakedObjectId),
            ],
        });
    }

    return tx;
}
