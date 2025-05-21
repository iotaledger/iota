// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_TYPE_ARG, IOTA_FRAMEWORK_ADDRESS, IOTA_CLOCK_OBJECT_ID } from '@iota/iota-sdk/utils';

interface CreateUnlockTimelockedObjectTransactionOptions {
    address: string;
    objectIds: string[];
}

export function createUnlockTimelockedObjectsTransaction({
    address,
    objectIds,
}: CreateUnlockTimelockedObjectTransactionOptions) {
    const ptb = new Transaction();
    const coins: { $kind: 'NestedResult'; NestedResult: [number, number] }[] = [];

    for (const objectId of objectIds) {
        const [unlock] = ptb.moveCall({
            target: `${IOTA_FRAMEWORK_ADDRESS}::timelock::unlock_with_clock`,
            typeArguments: [`${IOTA_FRAMEWORK_ADDRESS}::balance::Balance<${IOTA_TYPE_ARG}>`],
            arguments: [ptb.object(objectId), ptb.object(IOTA_CLOCK_OBJECT_ID)],
        });

        // Convert Balance to Coin
        const [coin] = ptb.moveCall({
            target: `${IOTA_FRAMEWORK_ADDRESS}::coin::from_balance`,
            typeArguments: [IOTA_TYPE_ARG],
            arguments: [ptb.object(unlock)],
        });

        coins.push(coin);
    }
    ptb.transferObjects(coins, ptb.pure.address(address));
    return ptb;
}
