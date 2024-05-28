// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import ActivityTile from '@/components/ActivityTile';
import { Activity, ActivityState } from '@/lib/interfaces';
import { useVirtualizer } from '@tanstack/react-virtual';
import React from 'react';

function StakingDashboardPage(): JSX.Element {
    const containerRef = React.useRef(null);
    const virtualizer = useVirtualizer({
        count: MOCK_ACTIVITIES.length,
        getScrollElement: () => containerRef.current,
        estimateSize: () => 100,
    });

    const virtualItems = virtualizer.getVirtualItems();

    return (
        <div
            className="flex flex-col items-center justify-center space-y-4 pt-12 h-full w-full"
        >
            <h1>Your Activity</h1>
            <div className="relative w-1/3 overflow-auto h-[50vh]" ref={containerRef}> 
                <div style={{
                    height: `${virtualizer.getTotalSize()}px`,
                    width: '100%',
                    position: 'relative',
                }}>
                    {virtualItems.map((virtualItem) => {
                        const activity = MOCK_ACTIVITIES[virtualItem.index];
                        return (
                            <div
                                key={virtualItem.key}
                                className="absolute w-full pr-4 pb-4"
                                style={{
                                    position: 'absolute',
                                    top: 0,
                                    left: 0,
                                    width: '100%',
                                    height: `${virtualItem.size}px`,
                                    transform: `translateY(${virtualItem.start}px)`,
                                }}
                            >
                                <ActivityTile activity={activity} />
                            </div>
                        );
                    })}
                </div>
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
