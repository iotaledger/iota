// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    DryRunTransactionBlockResponse,
    IotaGasData,
    IotaTransactionBlockResponse,
    TransactionEffects,
} from '@iota/iota-sdk/client';

import type { GasSummaryType } from '@/lib/types';

export function getGasSummary(
    transaction: IotaTransactionBlockResponse | DryRunTransactionBlockResponse,
): GasSummaryType {
    const { effects } = transaction;
    if (!effects) return null;
    const totalGas = getTotalGasUsed(effects);
    let sender: string | undefined;
    let gasData: IotaGasData | undefined;
    if ('transaction' in transaction && transaction.transaction?.data) {
        sender = transaction.transaction.data.sender;
        gasData = transaction.transaction.data.gasData;
    } else if ('input' in transaction) {
        sender = transaction.input?.sender;
        gasData = transaction.input?.gasData;
    }
    const owner = gasData?.owner ?? '';
    const isSponsored = !!owner && !!sender && owner !== sender;
    return {
        ...effects.gasUsed,
        ...gasData,
        owner,
        totalGas: totalGas?.toString(),
        isSponsored,
        gasUsed: effects.gasUsed,
    };
}
export function getTotalGasUsed(effects: TransactionEffects): bigint | undefined {
    const gas = effects?.gasUsed;
    if (!gas) return undefined;
    const { computationCost, storageCost, storageRebate } = gas;
    return BigInt(computationCost) + BigInt(storageCost) - BigInt(storageRebate);
}
