// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { useCurrentAccount } from '@iota/dapp-kit';
import { VirtualList, TransactionTile } from '@/components';
import { useQueryTransactionsByAddress } from '@iota/core';
import { getExtendedTransaction } from '@/lib/utils/transaction';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

export function TransactionsList() {
    const currentAccount = useCurrentAccount();
    const { data: transactions, error } = useQueryTransactionsByAddress(currentAccount?.address);

    if (error) {
        return <div>{error?.message}</div>;
    }

    const virtualItem = (rawTransaction: IotaTransactionBlockResponse): JSX.Element => {
        const transaction = getExtendedTransaction(rawTransaction, currentAccount?.address || '');
        return <TransactionTile transaction={transaction} />;
    };

    return <VirtualList items={transactions || []} estimateSize={() => 60} render={virtualItem} />;
}
