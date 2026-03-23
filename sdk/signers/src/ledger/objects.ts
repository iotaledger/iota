// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Transaction } from '@iota/iota-sdk/transactions';
import type { IotaClient } from '@iota/iota-sdk/client';
import { IotaMoveObject } from './bcs.js';

export const getInputObjects = async (transaction: Transaction, client: IotaClient) => {
    const data = transaction.getData();

    const gasObjectIds = data.gasData.payment?.map((object) => object.objectId) ?? [];
    const inputObjectIds = data.inputs
        .map((input) => {
            return input.$kind === 'Object' && input.Object.$kind === 'ImmOrOwnedObject'
                ? input.Object.ImmOrOwnedObject.objectId
                : null;
        })
        .filter((objectId): objectId is string => !!objectId);

    const objects = await client.multiGetObjects({
        ids: [...gasObjectIds, ...inputObjectIds],
        options: {
            showBcs: true,
            showPreviousTransaction: true,
            showStorageRebate: true,
            showOwner: true,
        },
    });

    // NOTE: We should probably get rid of this manual serialization logic in favor of using the
    // already serialized object bytes from the GraphQL API once there is more mainstream support
    // for it + we can enforce the transport type on the IOTA client.
    const bcsObjects = objects
        .map((object) => {
            if (object.error || !object.data || object.data.bcs?.dataType !== 'moveObject') {
                return null;
            }

            return IotaMoveObject.serialize({
                data: {
                    MoveObject: {
                        type: object.data.bcs.type,
                        version: object.data.bcs.version,
                        contents: object.data.bcs.bcsBytes,
                    },
                },
                owner: object.data.owner!,
                previousTransaction: object.data.previousTransaction!,
                storageRebate: object.data.storageRebate!,
            }).toBytes();
        })
        .filter((bcsBytes): bcsBytes is Uint8Array<ArrayBuffer> => !!bcsBytes);

    return { bcsObjects };
};
