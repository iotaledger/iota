// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionBlock, TransactionObjectArgument } from '@iota/iota-sdk/transactions';
import { IOTA_SYSTEM_STATE_OBJECT_ID, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

/**
 * The extended timelocked object is used to create a timelocked staking transaction.
 * The extended object contains objectIds of the objects that need to be merged during staking transaction and the split amount if the object needs to be split.
 */
export interface ExtendedTimelockObject {
    /**
     * The object id of the extended timelocked object is the first object id in the objectIds array.
     * The extended timelocked object will first undergo a merging process where all objects in the objectIds array are merged into the first one.
     */
    objectId: string;
    /**
     * The expiration timestamp is the same for all objects in the objectIds array.
     */
    expirationTimestamp: string;
    /**
     * The total locked amount is the sum of all locked amounts in the objectIds array
     */
    totalLockedAmount: bigint;
    /**
     * The array of object ids of the extended timelocked object.
     * The first element in the objectIds array is the principal object
     * and the rest are the objects that will be merged into the principal object.
     */
    objectIds: string[];
    /**
     * The label of the extended timelocked object.
     * The label is the same for all objects in the objectIds array.
     */
    label?: string;
    /**
     * The split amount of the extended timelocked object.
     * The split amount is the amount that will be split from the principal object.
     * Indicates if the object needs to be split during timelocked staking transaction.
     * Splitting occurs after merging.
     */
    splitAmount?: bigint;
}

export function createTimelockedStakeTransaction(
    timelockedObjects: ExtendedTimelockObject[],
    validatorAddress: string,
) {
    const tx = new TransactionBlock();
    /**
     * Create the transactions to merge the timelocked objects that need merging
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
     * Create the transactions to split the timelocked objects that need splitting.
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
     * Create the transactions to stake the timelocked objects
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
