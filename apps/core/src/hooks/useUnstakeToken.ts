// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function useStakeToken({ onSuccess }) {
    return useMutation({
        mutationFn: async ({
            tokenTypeArg,
            amount,
            validatorAddress,
        }: {
            tokenTypeArg: string;
            amount: bigint;
            validatorAddress: string;
        }) => {
            if (!validatorAddress || !amount || !tokenTypeArg || !signer) {
                throw new Error('Failed, missing required field');
            }

            const sentryTransaction = Sentry.startTransaction({
                name: 'stake',
            });
            try {
                const transactionBlock = createStakeTransaction(amount, validatorAddress);
                const tx = await signer.signAndExecuteTransaction({
                    transactionBlock,
                    options: {
                        showInput: true,
                        showEffects: true,
                        showEvents: true,
                    },
                });
                await signer.client.waitForTransaction({
                    digest: tx.digest,
                });
                return tx;
            } finally {
                sentryTransaction.finish();
            }
        },
        onSuccess: (_, { amount, validatorAddress }) => {
            ampli.stakedIota({
                stakedAmount: Number(amount / NANOS_PER_IOTA),
                validatorAddress: validatorAddress,
            });
        },
    });
}
