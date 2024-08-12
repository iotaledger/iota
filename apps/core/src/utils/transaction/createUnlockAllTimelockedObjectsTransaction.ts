// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionBlock } from '@iota/iota.js/transactions';
import { IOTA_TYPE_ARG, IOTA_FRAMEWORK_ADDRESS } from '@iota/iota.js/utils';

interface UnlockAllTimelockedObjectTransactionOptions {
    address: string;
    objectIds: string[];
}

export function createUnlockAllTimelockedObjectsTransaction({
    address,
    objectIds,
}: UnlockAllTimelockedObjectTransactionOptions) {
    const ptb = new TransactionBlock();
    const coins: { index: number; resultIndex: number; kind: 'NestedResult' }[] = [];

    for (const objectId of objectIds) {
        const [unlock] = ptb.moveCall({
            target: `${IOTA_FRAMEWORK_ADDRESS}::timelock::unlock`,
            typeArguments: [`${IOTA_FRAMEWORK_ADDRESS}::balance::Balance<${IOTA_TYPE_ARG}>`],
            arguments: [ptb.object(objectId)],
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
