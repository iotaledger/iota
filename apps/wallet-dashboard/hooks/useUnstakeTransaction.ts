// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createUnstakeTransaction } from '@iota/core';
import { useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useUnstakeTransaction(stakedIotaId: string) {
    const {
        mutateAsync: signAndExecuteTransactionBlock,
        error,
        isPending,
    } = useSignAndExecuteTransactionBlock();

    const { data: transaction } = useQuery({
        queryKey: ['unstake-transaction', stakedIotaId],
        queryFn: async () => {
            const transaction = createUnstakeTransaction(stakedIotaId);
            return transaction;
        },
        enabled: !!stakedIotaId,
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
