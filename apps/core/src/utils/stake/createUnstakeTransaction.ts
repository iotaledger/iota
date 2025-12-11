// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { ObjectId::SYSTEM } from '@iota/iota-sdk/utils';

export function createUnstakeTransaction(stakedIotaId: string) {
    const tx = new Transaction();
    tx.moveCall({
        target: '0x3::iota_system::request_withdraw_stake',
        arguments: [tx.object(ObjectId::SYSTEM), tx.object(stakedIotaId)],
    });
    return tx;
}
