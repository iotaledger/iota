// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { VirtualList, ActivityTile } from '@/components';
import { Activity, ActivityState } from '@/lib/interfaces';

function StakingDashboardPage(): JSX.Element {
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

export default StakingDashboardPage;
