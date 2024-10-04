// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCoinMetadata, createTokenTransferTransaction } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { CoinStruct } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

export function useSendCoinTransaction(
    coins: CoinStruct[],
    coinType: string,
    senderAddress: string,
    recipientAddress: string,
    amount: string,
    isPayAllIota: boolean,
) {
    const client = useIotaClient();
    const { data: coinMetadata } = useCoinMetadata();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'token-transfer-transaction',
            recipientAddress,
            amount,
            coins,
            coinType,
            coinMetadata?.decimals,
            senderAddress,
            isPayAllIota,
        ],
        queryFn: async () => {
            const transaction = createTokenTransferTransaction({
                coinType,
                coinDecimals: coinMetadata?.decimals || 0,
                to: recipientAddress,
                amount,
                coins,
                isPayAllIota,
            });

            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!recipientAddress && !!amount && !!coins && !!senderAddress && !!coinType,
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.getData().gasData.budget,
            };
        },
    });
}
