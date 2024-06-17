// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCoinMetadata, createTokenTransferTransaction } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { CoinStruct } from '@iota/iota.js/client';
import { useQuery } from '@tanstack/react-query';
import { COIN_DECIMALS } from '@/lib/constants';

export function useSendCoinTransaction(
    coin: CoinStruct,
    senderAddress: string,
    recipientAddress: string,
    amount: string,
) {
    const client = useIotaClient();
    const { data: coinMetadata } = useCoinMetadata();

    return useQuery({
        queryKey: [
            'token-transfer-transaction',
            recipientAddress,
            amount,
            coin,
            coinMetadata?.decimals,
            senderAddress,
            client,
        ],
        queryFn: async () => {
            const transaction = createTokenTransferTransaction({
                coinType: coin.coinType,
                coinDecimals: coinMetadata?.decimals || COIN_DECIMALS,
                to: recipientAddress,
                amount,
                coins: [coin],
                isPayAllIota: false,
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
