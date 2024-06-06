// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    ObjectOwner,
    IotaObjectChange,
    IotaTransactionBlockResponse,
} from '@iota/iota.js/client';
import type { TransactionBlock } from '@iota/iota.js/transactions';
import { normalizeStructTag, normalizeIotaAddress, parseStructTag } from '@iota/iota.js/utils';

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
    txb: TransactionBlock,
    options: {
        packageId: string;
    },
) {
    let transfers = 0;

    for (const tx of txb.blockData.transactions) {
        switch (tx.kind) {
            case 'TransferObjects':
                // Ensure that we are only transferring results of a claim
                if (!tx.objects.every((o) => o.kind === 'Result' || o.kind === 'NestedResult')) {
                    return false;
                }
                transfers++;
                break;
            case 'MoveCall':
                const [packageId, module, fn] = tx.target.split('::');

                if (packageId !== options.packageId) {
                    return false;
                }

                if (module !== 'zk_bag') {
                    return false;
                }

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

export function getAssetsFromTxnBlock({
    transactionBlock,
    address,
    isSent,
}: {
    transactionBlock: IotaTransactionBlockResponse;
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

    transactionBlock.balanceChanges?.forEach((change) => {
        const validAmountChange = isSent ? BigInt(change.amount) < 0n : BigInt(change.amount) > 0n;
        if (validAmountChange && isOwner(change.owner, normalizedAddress)) {
            balances.push({
                coinType: normalizeStructTag(change.coinType),
                amount: BigInt(change.amount),
            });
        }
    });

    transactionBlock.objectChanges?.forEach((change) => {
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
                    coins.push(change);
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
            nfts.push(change);
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
