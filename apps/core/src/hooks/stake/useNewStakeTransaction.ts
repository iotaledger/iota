// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { createStakeTransaction, getGasSummary } from '../../utils';
import { Transaction } from '@iota/iota-sdk/transactions';

export function useNewStakeTransaction(validator: string, amount: bigint, senderAddress: string) {
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'stake-transaction',
            '0xd17a0271bb4ae0a12af1a6d458805b6220603bb73a025d8da6d5c4cc848395e1',
            amount.toString(),
            senderAddress,
        ],
        queryFn: async () => {
            const transaction = createStakeTransaction(
                amount,
                '0xd17a0271bb4ae0a12af1a6d458805b6220603bb73a025d8da6d5c4cc848395e1',
            );
            transaction.setSender(senderAddress);
            const txBytes = await transaction.build({ client });
            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });
            return {
                txBytes,
                txDryRun,
            };
        },
        enabled: !!amount && !!validator && !!senderAddress,
        gcTime: 0,
        select: ({ txBytes, txDryRun }) => {
            return {
                transaction: Transaction.from(txBytes),
                gasSummary: getGasSummary(txDryRun),
            };
        },
    });
}
