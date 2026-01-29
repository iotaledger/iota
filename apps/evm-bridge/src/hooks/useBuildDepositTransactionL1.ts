// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { createDepositTransactionL1, getGasSummary } from '../lib/utils';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useNetworkVariables } from '../config';
import { CoinStruct } from '@iota/iota-sdk/client';

interface UseBuildDepositTransactionL1Props {
    amount: bigint; // Amount in nanos
    receivingAddress: string;
    coins?: CoinStruct[];
    coinType?: string;
    refetchInterval?: number;
}

export function useBuildDepositTransactionL1({
    receivingAddress,
    amount,
    coins,
    coinType = IOTA_TYPE_ARG,
    refetchInterval,
}: UseBuildDepositTransactionL1Props) {
    const senderAddress = useCurrentAccount()?.address as string;
    const client = useIotaClient();
    const variables = useNetworkVariables();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['l1-deposit-transaction', receivingAddress, amount.toString(), senderAddress],
        queryFn: async () => {
            if (!receivingAddress) {
                throw Error('Invalid input: receivingAddress is missing');
            }

            const transaction = createDepositTransactionL1({
                amount,
                receivingAddress,
                coins,
                coinType,
                chain: variables.chain,
            });

            transaction.setSender(senderAddress);
            const txBytes = await transaction.build({ client });
            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });
            if (txDryRun.effects.status.status !== 'success') {
                throw new Error(`Tx dry run failed: ${txDryRun.effects.status?.error}`);
            }
            return {
                txBytes,
                txDryRun,
            };
        },
        enabled: !!receivingAddress && !!amount && !!senderAddress && amount > 0n,
        gcTime: 0,
        select: ({ txBytes, txDryRun }) => {
            return {
                transaction: Transaction.from(txBytes),
                gasSummary: getGasSummary(txDryRun),
            };
        },
        refetchInterval,
    });
}
