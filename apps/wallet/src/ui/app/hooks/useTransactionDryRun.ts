// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Transaction } from '@iota/iota-sdk/transactions';
import { useQuery } from '@tanstack/react-query';

import { useAccountByAddress } from './useAccountByAddress';
import { useSigner } from './useSigner';
import type { ChainType } from '@iota/iota-sdk/client';

export function useTransactionDryRun(
    sender: string | undefined,
    transaction: Transaction,
    chain?: ChainType,
) {
    const { data: account } = useAccountByAddress(sender);
    const signer = useSigner(account || null);
    const response = useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['dryRunTransaction', transaction.getData()],
        queryFn: () => {
            return signer!.dryRunTransactionBlock({ transactionBlock: transaction, chain });
        },
        enabled: !!signer,
    });
    return response;
}
