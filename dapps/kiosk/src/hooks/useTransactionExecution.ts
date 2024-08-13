// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useSignTransactionBlock, useIotaClient } from '@iota/dapp-kit';
import { IotaTransactionBlockResponseOptions } from '@iota/iota-sdk/client';
import { TransactionBlock } from '@iota/iota-sdk/transactions';

// A helper to execute transactions by:
// 1. Signing them using the wallet
// 2. Executing them using the rpc provider
export function useTransactionExecution() {
    const provider = useIotaClient();

    // sign transaction from the wallet
    const { mutateAsync: signTransactionBlock } = useSignTransactionBlock();

    // tx: TransactionBlock
    const signAndExecute = async ({
        tx,
        options = { showEffects: true },
    }: {
        tx: TransactionBlock;
        options?: IotaTransactionBlockResponseOptions | undefined;
    }) => {
        // @ts-expect-error: This is an issue with type references not working together:
        const signedTx = await signTransactionBlock({ transactionBlock: tx });

        const res = await provider.executeTransactionBlock({
            transactionBlock: signedTx.transactionBlockBytes,
            signature: signedTx.signature,
            options,
        });

        const status = res.effects?.status?.status === 'success';

        if (status) return true;
        else throw new Error('Transaction execution failed.');
    };

    return { signAndExecute };
}
