// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionBlock } from '@iota/iota-sdk/transactions';
import { IOTA_SYSTEM_STATE_OBJECT_ID, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

/**
 * The grouped timelocked object is used to create a timelocked staking transaction.
 * The grouped object contains mergeObjectIds of the objects that need to be merged during staking transaction and the split amount if the object needs to be split.
 */
export interface GroupedTimelockObject {
    /**
     * The object id of the grouped timelocked object.
     */
    objectId: string;
    /**
     * The expiration timestamp is the same for all objects in the mergeObjectIds array.
     */
    expirationTimestamp: string;
    /**
     * The total locked amount is the sum of all locked amounts in the mergeObjectIds array
     */
    totalLockedAmount: bigint;
    /**
     * The array of object ids of the grouped timelocked object.
     */
    mergeObjectIds: string[];
    /**
     * The label of the timelocked objects in mergeObjectIds.
     */
    label?: string;
    /**
     * The split amount of the timelocked object.
     */
    splitAmount?: bigint;
}

export function createTimelockedStakeTransaction(
    timelockedObjects: GroupedTimelockObject[],
    validatorAddress: string,
) {
    const tx = new TransactionBlock();

    // Create the transactions to merge the timelocked objects that need merging
    const mergeObjects = timelockedObjects.filter((obj) => obj.mergeObjectIds.length > 0);

    for (const mergeObject of mergeObjects) {
        // create an array of objectIds to be merged without the first element because first element is the principal object and its id is contained in mergeObject.objectId
        const mergeObjectIds = mergeObject.mergeObjectIds.map((objectId) => tx.object(objectId));
        tx.moveCall({
            target: `0x02::timelock::join_vec`,
            typeArguments: [`${IOTA_TYPE_ARG}`],
            arguments: [
                tx.object(mergeObject.objectId),
                tx.makeMoveVec({ objects: mergeObjectIds }),
            ],
        });
    }

    // Create the transactions to split the timelocked objects that need splitting.
    const splitTimelockedObjects: GroupedTimelockObject[] = timelockedObjects.filter(
        (obj) => obj.splitAmount !== undefined && obj.splitAmount > 0,
    );
    const splitTimelockedObjectTransactions = splitTimelockedObjects.map((obj) => {
        const [splitTx] = tx.moveCall({
            target: `0x02::timelock::split`,
            typeArguments: [`${IOTA_TYPE_ARG}`],
            arguments: [tx.object(obj.objectId), tx.pure.u64(obj.splitAmount!)],
        });
        return tx.object(splitTx);
    });

    // Create the transactions to stake the timelocked objects
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
