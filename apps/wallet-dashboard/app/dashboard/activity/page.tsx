// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React, { useMemo } from 'react';
import { useCurrentAccount } from '@mysten/dapp-kit';
import { VirtualList, ActivityTile } from '@/components';
import { Activity, ActivityState } from '@/lib/interfaces';
import { useQueryTransactionsByAddress } from '@/hooks/useQueryTransactionsByAddress';
import { getTransactionActivity } from '@/lib/utils/activity';

function StakingDashboardPage(): JSX.Element {
    const containerRef = React.useRef(null);
    const currentAccount = useCurrentAccount();
    const { data: txs, error } = useQueryTransactionsByAddress(currentAccount?.address);

    const mapped = useMemo(() => {
        return txs?.map((tx) => getTransactionActivity(tx, currentAccount?.address)) || [];
    }, [currentAccount?.address, txs]);

    if (error) {
        return <div>{(error as Error)?.message}</div>;
    }

    const virtualItem = (activity: Activity): JSX.Element => (
        <ActivityTile key={activity.timestamp} activity={activity} />
    );

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4 pt-12">
            <h1>Your Activity</h1>
            <div className="flex w-1/2">
                <VirtualList
                    items={MOCK_ACTIVITIES}
                    estimateSize={() => 100}
                    render={virtualItem}
                />
            </div>
        </div>
    );
}

const MOCK_ACTIVITIES: Activity[] = [
    {
        action: 'Send',
        state: ActivityState.Successful,
        timestamp: 1716538921485,
    },
    {
        action: 'Transaction',
        state: ActivityState.Successful,
        timestamp: 1715868828552,
    },
    {
        action: 'Send',
        state: ActivityState.Successful,
        timestamp: 1712186639729,
    },
    {
        action: 'Rewards',
        state: ActivityState.Successful,
        timestamp: 1715868828552,
    },
    {
        action: 'Receive',
        state: ActivityState.Successful,
        timestamp: 1712186639729,
    },
    {
        action: 'Transaction',
        state: ActivityState.Successful,
        timestamp: 1715868828552,
    },
    {
        action: 'Send',
        state: ActivityState.Failed,
        timestamp: 1712186639729,
    },
    {
        action: 'Send',
        state: ActivityState.Successful,
        timestamp: 1716538921485,
    },
    {
        action: 'Transaction',
        state: ActivityState.Successful,
        timestamp: 1715868828552,
    },
    {
        action: 'Send',
        state: ActivityState.Successful,
        timestamp: 1712186639729,
    },
    {
        action: 'Rewards',
        state: ActivityState.Successful,
        timestamp: 1715868828552,
    },
    {
        action: 'Receive',
        state: ActivityState.Successful,
        timestamp: 1712186639729,
    },
    {
        action: 'Transaction',
        state: ActivityState.Successful,
        timestamp: 1715868828552,
    },
    {
        action: 'Send',
        state: ActivityState.Failed,
        timestamp: 1712186639729,
    },
];

export default ActivityPage;
