// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionBlock } from '@iota/iota.js/transactions';
import { IOTA_SYSTEM_STATE_OBJECT_ID } from '@iota/iota.js/utils';

export function createUnstakeTransaction(stakedIotaId: string) {
    const tx = new TransactionBlock();
    tx.moveCall({
        target: '0x3::iota_system::request_withdraw_stake',
        arguments: [tx.object(IOTA_SYSTEM_STATE_OBJECT_ID), tx.object(stakedIotaId)],
    });
    return tx;
}
