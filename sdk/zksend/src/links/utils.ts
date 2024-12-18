// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    ObjectOwner,
    IotaObjectChange,
    IotaTransactionBlockResponse,
} from '@iota/iota-sdk/client';
import type { Transaction } from '@iota/iota-sdk/transactions';
import { normalizeStructTag, normalizeIotaAddress, parseStructTag } from '@iota/iota-sdk/utils';

// eslint-disable-next-line import/no-cycle

export interface LinkAssets {
    balances: {
        coinType: string;
        amount: bigint;
    }[];

    nfts: {
        objectId: string;
        type: string;
        version: string;
        digest: string;
    }[];

    coins: {
        objectId: string;
        type: string;
        version: string;
        digest: string;
    }[];
}

export function isClaimTransaction(
    tx: Transaction,
    options: {
        packageId: string;
    },
) {
    let transfers = 0;

    for (const command of tx.getData().commands) {
        switch (command.$kind) {
            case 'TransferObjects':
                // Ensure that we are only transferring results of a claim
                if (
                    !command.TransferObjects.objects.every(
                        (o) => o.$kind === 'Result' || o.$kind === 'NestedResult',
                    )
                ) {
                    return false;
                }
                transfers++;
                break;
            case 'MoveCall':
                if (command.MoveCall.package !== options.packageId) {
                    return false;
                }

                if (command.MoveCall.module !== 'zk_bag') {
                    return false;
                }
                const fn = command.MoveCall.function;
                if (
                    fn !== 'init_claim' &&
                    fn !== 'reclaim' &&
                    fn !== 'claim' &&
                    fn !== 'finalize'
                ) {
                    return false;
                }
                break;
            default:
                return false;
        }
    }

    return transfers === 1;
}

export function getAssetsFromTransaction({
    transaction,
    address,
    isSent,
}: {
    transaction: IotaTransactionBlockResponse;
    address: string;
    isSent: boolean;
}): LinkAssets {
    const normalizedAddress = normalizeIotaAddress(address);
    const balances: {
        coinType: string;
        amount: bigint;
    }[] = [];

    const nfts: {
        objectId: string;
        type: string;
        version: string;
        digest: string;
    }[] = [];

    const coins: {
        objectId: string;
        type: string;
        version: string;
        digest: string;
    }[] = [];

    transaction.balanceChanges?.forEach((change) => {
        const validAmountChange = isSent ? BigInt(change.amount) < 0n : BigInt(change.amount) > 0n;
        if (validAmountChange && isOwner(change.owner, normalizedAddress)) {
            balances.push({
                coinType: normalizeStructTag(change.coinType),
                amount: BigInt(change.amount),
            });
        }
    });

    transaction.objectChanges?.forEach((change) => {
        if (!isObjectOwner(change, normalizedAddress, isSent)) {
            return;
        }

        if ('objectType' in change) {
            const type = parseStructTag(change.objectType);

            if (
                type.address === normalizeIotaAddress('0x2') &&
                type.module === 'coin' &&
                type.name === 'Coin'
            ) {
                if (
                    change.type === 'created' ||
                    change.type === 'transferred' ||
                    change.type === 'mutated'
                ) {
                    coins.push({
                        ...change,
                        type: change.objectType,
                    });
                }
                return;
            }
        }

        if (
            isObjectOwner(change, normalizedAddress, isSent) &&
            (change.type === 'created' ||
                change.type === 'transferred' ||
                change.type === 'mutated')
        ) {
            nfts.push({
                objectId: change.objectId,
                type: change.objectType,
                version: change.version,
                digest: change.digest,
            });
        }
    });

    return {
        balances,
        nfts,
        coins,
    };
}

function getObjectOwnerFromObjectChange(objectChange: IotaObjectChange, isSent: boolean) {
    if (isSent) {
        return 'owner' in objectChange ? objectChange.owner : null;
    }

    return 'recipient' in objectChange ? objectChange.recipient : null;
}

function isObjectOwner(objectChange: IotaObjectChange, address: string, isSent: boolean) {
    const owner = getObjectOwnerFromObjectChange(objectChange, isSent);

    if (isSent) {
        return owner && typeof owner === 'object' && 'AddressOwner' in owner;
    }

    return ownedAfterChange(objectChange, address);
}

export function ownedAfterChange(
    objectChange: IotaObjectChange,
    address: string,
): objectChange is Extract<IotaObjectChange, { type: 'created' | 'transferred' | 'mutated' }> {
    if (objectChange.type === 'transferred' && isOwner(objectChange.recipient, address)) {
        return true;
    }

    if (
        (objectChange.type === 'created' || objectChange.type === 'mutated') &&
        isOwner(objectChange.owner, address)
    ) {
        return true;
    }

    return false;
}

export function isOwner(owner: ObjectOwner, address: string): owner is { AddressOwner: string } {
    return (
        owner &&
        typeof owner === 'object' &&
        'AddressOwner' in owner &&
        normalizeIotaAddress(owner.AddressOwner) === address
    );
}
