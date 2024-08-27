// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionBlock, TransactionObjectArgument } from '@iota/iota-sdk/transactions';
import { IOTA_SYSTEM_STATE_OBJECT_ID, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export interface ExtendedTimelockObject {
    objectId: string;
    expirationTimestamp: string;
    totalLockedAmount: bigint;
    objectIds: string[];
    label?: string;
    splitAmount?: bigint;
}

export function createTimelockedStakeTransaction(
    timelockedObjects: ExtendedTimelockObject[],
    validatorAddress: string,
) {
    const tx = new TransactionBlock();
    /**
     * Create tranasctions to merge timelocked objects that need merging
     */
    const mergeObjects = timelockedObjects.filter((obj) => obj.objectIds.length > 1);

    for (const mergeObject of mergeObjects) {
        // create an array of objectIds to be merged without the first element because first element is the principal object and its id is contained in mergeObject.objectId
        const mergeObjectIds = mergeObject.objectIds
            .slice(1)
            .map((objectId) => tx.object(objectId));
        tx.moveCall({
            target: `0x02::timelock::join_vec`,
            typeArguments: [`${IOTA_TYPE_ARG}`],
            arguments: [
                tx.object(mergeObject.objectId),
                tx.makeMoveVec({ objects: mergeObjectIds }),
            ],
        });
    }
    /**
     * Create transactions to split timelocked objects that need splitting.
     */
    const splitTimelockedObjects: ExtendedTimelockObject[] = timelockedObjects.filter(
        (obj) => obj.splitAmount !== undefined && obj.splitAmount > 0,
    );
    const splitTimelockedObjectTransactions: TransactionObjectArgument[] = [];
    splitTimelockedObjects.forEach((obj) => {
        const [splitTx] = tx.moveCall({
            target: `0x02::timelock::split`,
            typeArguments: [`${IOTA_TYPE_ARG}`],
            arguments: [tx.object(obj.objectId), tx.pure.u64(obj.splitAmount!)],
        });
        splitTimelockedObjectTransactions.push(tx.object(splitTx));
    });

    /**
     * Create transactions to stake the timelocked objects
     */
    const stakingReadyObjects = timelockedObjects
        .filter((obj) => obj.splitAmount === undefined || obj.splitAmount === BigInt(0))
        .map((obj) => tx.object(obj.objectId));
    tx.moveCall({
        target: `0x3::timelocked_staking::request_add_stake_mul_bal`,
        arguments: [
            tx.sharedObjectRef({
                objectId: IOTA_SYSTEM_STATE_OBJECT_ID,
                initialSharedVersion: 1,
                mutable: true,
            }),
            tx.makeMoveVec({
                objects: [...splitTimelockedObjectTransactions, ...stakingReadyObjects],
            }),
            tx.pure.address(validatorAddress),
        ],
    });

    return tx;
}
