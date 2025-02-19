// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { createStakeTransaction, getGasSummary } from '../../utils';

export function useNewStakeTransaction(validator: string, amount: bigint, senderAddress: string) {
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['stake-transaction', validator, amount.toString(), senderAddress],
        queryFn: async () => {
            const transaction = createStakeTransaction(amount, validator);
            transaction.setSender(senderAddress);
            const builtTransaction = await transaction.build({ client });
            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: builtTransaction,
            });
            return {
                transaction,
                txDryRun,
            };
        },
        enabled: !!amount && !!validator && !!senderAddress,
        gcTime: 0,
        select: ({ transaction, txDryRun }) => {
            return {
                transaction,
                gasSummary: getGasSummary(txDryRun),
            };
        },
    });
}
