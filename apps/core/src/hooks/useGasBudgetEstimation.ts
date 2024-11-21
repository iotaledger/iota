// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { CoinStruct } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';
import { createTokenTransferTransaction } from '../utils';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useFormatCoin } from './useFormatCoin';
import { GAS_SYMBOL } from '../constants';

interface UseGasBudgetEstimationOptions {
    coinDecimals: number;
    coins: CoinStruct[];
    activeAddress: string;
    to: string;
    amount: string;
    isPayAllIota: boolean;
}

export function useGasBudgetEstimation({
    coinDecimals,
    coins,
    activeAddress,
    to,
    amount,
    isPayAllIota,
}: UseGasBudgetEstimationOptions) {
    const client = useIotaClient();
    const { data: gasBudget } = useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'transaction-gas-budget-estimate',
            {
                to,
                amount,
                coins,
                activeAddress,
                coinDecimals,
            },
        ],
        queryFn: async () => {
            if (!amount || !to || !coins || !activeAddress) {
                return null;
            }

            const tx = createTokenTransferTransaction({
                to,
                amount: amount,
                coinType: IOTA_TYPE_ARG,
                coinDecimals,
                isPayAllIota: isPayAllIota,
                coins,
            });

            tx.setSender(activeAddress);
            await tx.build({ client });
            return tx.getData().gasData.budget;
        },
    });

    const [formattedGas] = useFormatCoin(gasBudget, IOTA_TYPE_ARG);

    return formattedGas ? formattedGas + ' ' + GAS_SYMBOL : '--';
}
