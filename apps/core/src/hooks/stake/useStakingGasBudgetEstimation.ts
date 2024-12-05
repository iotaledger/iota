// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { createStakeTransaction } from '../../utils';

interface UseStakingGasBudgetEstimationOptions {
    senderAddress: string | null;
    validatorAddress: string;
    amount: bigint;
}

export function useStakingGasBudgetEstimation({
    senderAddress,
    validatorAddress,
    amount,
}: UseStakingGasBudgetEstimationOptions) {
    const client = useIotaClient();

    const transaction = useMemo(() => {
        return createStakeTransaction(amount, validatorAddress);
    }, [amount, validatorAddress]);

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'staking-tx-gas-budget-estimate',
            senderAddress,
            validatorAddress,
            transaction.getData(),
        ],
        queryFn: async () => {
            if (!senderAddress || !transaction) {
                return null;
            }

            transaction.setSender(senderAddress);

            await transaction.build({ client });

            return transaction.getData().gasData.budget;
        },
    });
}
