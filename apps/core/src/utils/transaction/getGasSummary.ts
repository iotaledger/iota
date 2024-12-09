// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import type {
    DryRunTransactionBlockResponse,
    IotaTransactionBlockResponse,
    TransactionEffects,
} from '@iota/iota-sdk/client';
import type { GasSummaryType } from '../../types';

export function getGasSummary(
    transaction: IotaTransactionBlockResponse | DryRunTransactionBlockResponse,
): GasSummaryType {
    const { effects } = transaction;
    if (!effects) return null;
    const totalGas = getTotalGasUsed(effects);

    const sender = 'transaction' in transaction ? transaction.transaction?.data.sender : undefined;

    const gasData = 'transaction' in transaction ? transaction.transaction?.data.gasData : {};

    const owner =
        'transaction' in transaction
            ? transaction.transaction?.data.gasData.owner
            : typeof effects.gasObject.owner === 'object' &&
                'AddressOwner' in effects.gasObject.owner
              ? effects.gasObject.owner.AddressOwner
              : '';

    return {
        ...effects.gasUsed,
        ...gasData,
        owner,
        totalGas: totalGas?.toString(),
        isSponsored: !!owner && !!sender && owner !== sender,
        gasUsed: transaction?.effects!.gasUsed,
    };
}

export function getTotalGasUsed(effects: TransactionEffects): bigint | undefined {
    const gasSummary = effects?.gasUsed;
    return gasSummary
        ? BigInt(gasSummary.computationCost) +
              BigInt(gasSummary.storageCost) -
              BigInt(gasSummary.storageRebate)
        : undefined;
}
