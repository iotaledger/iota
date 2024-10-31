// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { useCurrentAccount } from '@iota/dapp-kit';
import { VirtualList, TransactionTile } from '@/components';
import { useQueryTransactionsByAddress } from '@iota/core';
import { getExtendedTransaction } from '@/lib/utils/transaction';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

function ActivityPage(): JSX.Element {
    const currentAccount = useCurrentAccount();
    const { data: transactions, error } = useQueryTransactionsByAddress(currentAccount?.address);

    if (error) {
        return <div>{error?.message}</div>;
    }

    const virtualItem = (rawTransaction: IotaTransactionBlockResponse): JSX.Element => {
        const transaction = getExtendedTransaction(rawTransaction, currentAccount?.address || '');
        return <TransactionTile transaction={transaction} />;
    };

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4 pt-12">
            <h1>Your Activity</h1>
            <div className="flex w-1/2">
                <VirtualList
                    items={transactions || []}
                    estimateSize={() => 100}
                    render={virtualItem}
                />
            </div>
        </div>
    );
}

export default ActivityPage;
