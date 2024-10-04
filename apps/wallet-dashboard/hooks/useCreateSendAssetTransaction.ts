// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useMutation } from '@tanstack/react-query';

export function useCreateSendAssetTransaction(
    objectId: string,
    onSuccess: () => void,
    onError: (error: unknown) => void,
) {
    const { mutateAsync: signAndExecuteTransactionBlock } = useSignAndExecuteTransactionBlock();

    const mutation = useMutation({
        mutationFn: async (to: string) => {
            if (!to) {
                throw new Error('Missing data');
            }

            const tx = new Transaction();
            tx.transferObjects([tx.object(objectId)], to);

            return signAndExecuteTransactionBlock({
                transactionBlock: tx,
                options: {
                    showInput: true,
                    showEffects: true,
                    showEvents: true,
                },
            });
        },
        onSuccess,
        onError,
    });

    return { mutation };
}
