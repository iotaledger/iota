// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { useCurrentAccount } from '@iota/dapp-kit';
import { VirtualList, ActivityTile } from '@/components';
import { Activity } from '@/lib/interfaces';
import { useQueryTransactionsByAddress } from '@iota/core';
import { getTransactionActivity } from '@/lib/utils/activity';

function ActivityPage(): JSX.Element {
    const currentAccount = useCurrentAccount();
    const { data: txs, error } = useQueryTransactionsByAddress(currentAccount?.address);

    const activities = (() => {
        if (!currentAccount?.address || !txs?.length) {
            return [];
        }
        return txs.map((tx) => getTransactionActivity(tx, currentAccount.address));
    })();

    if (error) {
        return <div>{error?.message}</div>;
    }

    const virtualItem = (activity: Activity): JSX.Element => (
        <ActivityTile key={activity.timestamp} activity={activity} />
    );

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4 pt-12">
            <h1>Your Activity</h1>
            <div className="flex w-1/2">
                <VirtualList items={activities} estimateSize={() => 100} render={virtualItem} />
            </div>
        </div>
    );
}

export default ActivityPage;
