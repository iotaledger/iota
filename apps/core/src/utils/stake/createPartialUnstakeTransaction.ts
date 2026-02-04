// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_SYSTEM_STATE_OBJECT_ID } from '@iota/iota-sdk/utils';

export function createPartialUnstakeTransaction(stakedIotaId: string, unstakeAmount: bigint) {
    const tx = new Transaction();
    const splitStakedIota = tx.moveCall({
        target: '0x3::staking_pool::split',
        arguments: [tx.object(stakedIotaId), tx.pure.u64(unstakeAmount)],
    });
    tx.moveCall({
        target: '0x3::iota_system::request_withdraw_stake',
        arguments: [tx.object(IOTA_SYSTEM_STATE_OBJECT_ID), splitStakedIota],
    });
    return tx;
}
