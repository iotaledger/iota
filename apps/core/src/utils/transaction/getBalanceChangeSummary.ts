// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    type DryRunTransactionBlockResponse,
    type ObjectOwner,
    type IotaTransactionBlockResponse,
} from '@iota/iota-sdk/client';
import { normalizeIotaObjectId, parseStructTag } from '@iota/iota-sdk/utils';

export type BalanceChange = {
    coinType: string;
    amount: string;
    recipient?: string;
    owner?: string;
    unRecognizedToken: boolean;
};

export type BalanceChangeByOwner = Record<string, BalanceChange[]>;
export type BalanceChangeSummary = BalanceChangeByOwner | null;

function getOwnerAddress(owner: ObjectOwner): string {
    if (typeof owner === 'object') {
        if ('AddressOwner' in owner) {
            return owner.AddressOwner;
        } else if ('ObjectOwner' in owner) {
            return owner.ObjectOwner;
        } else if ('Shared' in owner) {
            return 'Shared';
        }
    }
    return '';
}

export const getBalanceChangeSummary = (
    transaction: DryRunTransactionBlockResponse | IotaTransactionBlockResponse,
    recognizedPackagesList: string[],
) => {
    const { balanceChanges, effects } = transaction;
    if (!balanceChanges || !effects) return null;

    const normalizedRecognizedPackages = recognizedPackagesList.map((itm) =>
        normalizeIotaObjectId(itm),
    );
    const balanceChangeByOwner = {};
    return balanceChanges.reduce((acc, balanceChange) => {
        const amount = BigInt(balanceChange.amount);
        const owner = getOwnerAddress(balanceChange.owner);

        const recipient = balanceChanges.find(
            (bc) => balanceChange.coinType === bc.coinType && amount === BigInt(bc.amount) * -1n,
        );
        const { address: packageId } = parseStructTag(balanceChange.coinType);

        const recipientAddress = recipient?.owner ? getOwnerAddress(recipient?.owner) : undefined;

        const summary = {
            coinType: balanceChange.coinType,
            amount: amount.toString(),
            recipient: recipientAddress,
            owner,
            unRecognizedToken: !normalizedRecognizedPackages.includes(packageId),
        };

        acc[owner] = (acc[owner] ?? []).concat(summary);
        return acc;
    }, balanceChangeByOwner as BalanceChangeByOwner);
};

export const getRecognizedUnRecognizedTokenChanges = (changes: BalanceChange[]) => {
    const recognizedTokenChanges: BalanceChange[] = [];
    const unRecognizedTokenChanges: BalanceChange[] = [];
    for (const change of changes) {
        if (change.unRecognizedToken) {
            unRecognizedTokenChanges.push(change);
        } else {
            recognizedTokenChanges.push(change);
        }
    }
    return { recognizedTokenChanges, unRecognizedTokenChanges };
};
