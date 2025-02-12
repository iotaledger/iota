// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { type CoinStruct } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';
import { useCoinMetadata } from './useFormatCoin';
import { createTokenTransferTransaction } from '../utils';

interface SendCoinTransactionParams {
    coins: CoinStruct[];
    coinType: string;
    senderAddress: string;
    recipientAddress: string;
    amount: string;
}

export function useSendCoinTransaction({
    coins,
    coinType,
    senderAddress,
    recipientAddress,
    amount,
}: SendCoinTransactionParams) {
    const client = useIotaClient();
    const { data: coinMetadata } = useCoinMetadata(coinType);
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
        ],
        queryFn: async () => {
            const transaction = createTokenTransferTransaction({
                coinType,
                coinDecimals: coinMetadata?.decimals || 0,
                to: recipientAddress,
                amount,
                coins,
            });

            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!recipientAddress && !!amount && !!coins && !!senderAddress && !!coinType,
        gcTime: 0,
    });
}
