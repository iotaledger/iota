// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import type {
    DryRunTransactionBlockResponse,
    IotaGasData,
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
    let sender = undefined;
    let owner = '';
    let gasData = {} as IotaGasData;
    if ('transaction' in transaction && transaction.transaction?.data) {
        sender = transaction.transaction?.data.sender;
        gasData = transaction.transaction.data.gasData;
    } else if ('input' in transaction) {
        sender = transaction.input.sender;
        gasData = transaction.input.gasData;
    }
    owner = gasData?.owner ?? '';

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
