// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_TYPE_ARG, IOTA_FRAMEWORK_ADDRESS } from '@iota/iota-sdk/utils';

interface CreateUnlockTimelockedObjectTransactionOptions {
    address: string;
    objectIds: string[];
    isClockTimestampEnabled?: boolean;
}

export function createUnlockTimelockedObjectsTransaction({
    address,
    objectIds,
    isClockTimestampEnabled,
}: CreateUnlockTimelockedObjectTransactionOptions) {
    const ptb = new Transaction();
    const coins: { $kind: 'NestedResult'; NestedResult: [number, number] }[] = [];

    for (const objectId of objectIds) {
        let unlockTarget = `${IOTA_FRAMEWORK_ADDRESS}::timelock::unlock`;
        const unlockArgs = [ptb.object(objectId)];

        if (isClockTimestampEnabled) {
            unlockTarget = `${IOTA_FRAMEWORK_ADDRESS}::timelock::unlock_with_clock`;
            unlockArgs.push(ptb.object(`0x06`));
        }

        const [unlock] = ptb.moveCall({
            target: unlockTarget,
            typeArguments: [`${IOTA_FRAMEWORK_ADDRESS}::balance::Balance<${IOTA_TYPE_ARG}>`],
            arguments: unlockArgs,
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
