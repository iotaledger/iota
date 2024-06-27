// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCoinMetadata, createTokenTransferTransaction } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { CoinBalance, CoinStruct } from '@iota/iota.js/client';
import { useQuery } from '@tanstack/react-query';

export function useSendCoinTransaction(
    coin: CoinBalance,
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
            coin,
            coinMetadata?.decimals,
            senderAddress,
            isPayAllIota,
        ],
        queryFn: async () => {
            const coinStruct = coin as unknown as CoinStruct;
            const transaction = createTokenTransferTransaction({
                coinType: coin.coinType,
                coinDecimals: coinMetadata?.decimals || 0,
                to: recipientAddress,
                amount,
                coins: [coinStruct],
                isPayAllIota,
            });

            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!recipientAddress && !!amount && !!coin && !!senderAddress,
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.blockData.gasConfig.budget,
            };
        },
    });
}
