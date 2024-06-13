// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createUnstakeTransaction } from '@iota/core';
import { useIotaClient, useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useUnstakeTransaction(stakedIotaId: string, senderAddress: string) {
    const client = useIotaClient();
    const {
        mutateAsync: signAndExecuteTransactionBlock,
        error,
        isPending,
    } = useSignAndExecuteTransactionBlock();

    const { data: transaction } = useQuery({
        queryKey: ['unstake-transaction', stakedIotaId, senderAddress, client],
        queryFn: async () => {
            const transaction = createUnstakeTransaction(stakedIotaId);
            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!stakedIotaId && !!senderAddress,
        gcTime: 0,
    });

    const gasBudget = transaction?.blockData.gasConfig.budget?.toString() || '';

    const unstake = async (cb: () => void) => {
        if (!transaction) return;
        await signAndExecuteTransactionBlock({
            transactionBlock: transaction,
        });
        cb();
    };

    return {
        transaction,
        gasBudget,
        unstake,
        error,
        isPending,
    };
}
