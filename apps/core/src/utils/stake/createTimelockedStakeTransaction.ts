// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionBlock, TransactionObjectArgument } from '@iota/iota-sdk/transactions';
import { IOTA_SYSTEM_STATE_OBJECT_ID, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export interface VestingObject {
    objectId: string;
    expirationTimestamp: string;
    totalLockedAmount: bigint;
    objectIds: string[];
    label?: string;
    splitAmount?: bigint;
}

export function createTimelockedStakeTransaction(
    vestingObjects: VestingObject[],
    validatorAddress: string,
) {
    const tx = new TransactionBlock();
    /**
     * Create tranasctions to merge Vesting Objects that need merging
     */
    const mergeObjects = vestingObjects.filter((obj) => obj.objectIds.length > 1);

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
     * Create tranasctions to split Vesting Objects that need splitting.
     */
    const splitVestingObject: VestingObject[] = vestingObjects.filter(
        (obj) => obj.splitAmount !== undefined && obj.splitAmount > 0,
    );
    const splitVestingTransactions: TransactionObjectArgument[] = [];
    splitVestingObject.forEach((obj) => {
        const [splitTx] = tx.moveCall({
            target: `0x02::timelock::split`,
            typeArguments: [`${IOTA_TYPE_ARG}`],
            arguments: [tx.object(obj.objectId), tx.pure.u64(obj.splitAmount!)],
        });
        splitVestingTransactions.push(tx.object(splitTx));
    });

    /**
     * Create transactions to stake the vesting objects
     */
    const stakingReadyObjects = vestingObjects
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
            tx.makeMoveVec({ objects: [...splitVestingTransactions, ...stakingReadyObjects] }), // add the split object to the array of timelocked objects
            tx.pure.address(validatorAddress),
        ],
    });

    return tx;
}
