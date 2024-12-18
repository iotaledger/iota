// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { DescriptionItem } from '_pages/approval-request/transaction-request/DescriptionList';
import { getGasSummary, useCoinMetadata, useFormatCoin } from '@iota/core';
import { type DryRunTransactionBlockResponse } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';

interface GasFeesSummaryProps {
    transaction?: DryRunTransactionBlockResponse;
    feePercentage?: number;
    accessFees?: string;
    accessFeeType?: string;
}

export function GasFeesSummary({
    transaction,
    feePercentage,
    accessFees,
    accessFeeType,
}: GasFeesSummaryProps) {
    const gasSummary = useMemo(() => {
        if (!transaction) return null;
        return getGasSummary(transaction);
    }, [transaction]);
    const totalGas = gasSummary?.totalGas;
    const [gasAmount, gasSymbol] = useFormatCoin(totalGas, IOTA_TYPE_ARG);

    const { data: accessFeeMetadata } = useCoinMetadata(accessFeeType);

    return (
        <div className="flex flex-col gap-2 rounded-xl border border-solid border-hero-darkest/20 px-5 py-3">
            <DescriptionItem
                title={
                    <Text variant="bodySmall" weight="medium" color="steel-dark">
                        Access Fees ({feePercentage ? `${feePercentage * 100}%` : '--'})
                    </Text>
                }
            >
                <Text variant="bodySmall" weight="medium" color="steel-darker">
                    {accessFees ?? '--'}
                    {accessFeeMetadata?.symbol ? ` ${accessFeeMetadata.symbol}` : ''}
                </Text>
            </DescriptionItem>

            <div className="h-px w-full bg-gray-40" />

            <DescriptionItem
                title={
                    <Text variant="bodySmall" weight="medium" color="steel-dark">
                        Estimated Gas Fee
                    </Text>
                }
            >
                <Text variant="bodySmall" weight="medium" color="steel-darker">
                    {gasAmount ? `${gasAmount} ${gasSymbol}` : '--'}
                </Text>
            </DescriptionItem>
        </div>
    );
}
