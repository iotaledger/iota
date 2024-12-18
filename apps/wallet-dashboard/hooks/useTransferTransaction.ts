// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useMutation } from '@tanstack/react-query';
import { useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { Transaction } from '@iota/iota-sdk/transactions';

export function useTransferTransactionMutation() {
    const iotaClient = useIotaClient();
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();

    return useMutation({
        mutationFn: async (transaction: Transaction) => {
            const executed = await signAndExecuteTransaction({
                transaction,
            });

            const tx = await iotaClient.waitForTransaction({
                digest: executed.digest,
            });

            return tx;
        },
    });
}
