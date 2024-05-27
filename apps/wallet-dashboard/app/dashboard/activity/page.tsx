// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import ActivityTile from '@/components/ActivityTile';
import { useVirtualizer } from '@tanstack/react-virtual';
import React from 'react';

function StakingDashboardPage(): JSX.Element {
    const MOCK_ACTIVITIES = [
        {
            action: 'Send',
            success: true,
            timestamp: 1716538921485,
        },
        {
            action: 'Transaction',
            success: true,
            timestamp: 1715868828552,
        },
        {
            action: 'Send',
            success: true,
            timestamp: 1712186639729,
        },
        {
            action: 'Rewards',
            success: true,
            timestamp: 1715868828552,
        },
        {
            action: 'Receive',
            success: true,
            timestamp: 1712186639729,
        },
        {
            action: 'Transaction',
            success: true,
            timestamp: 1715868828552,
        },
        {
            action: 'Send',
            success: false,
            timestamp: 1712186639729,
        },
        {
            action: 'Send',
            success: true,
            timestamp: 1716538921485,
        },
        {
            action: 'Transaction',
            success: true,
            timestamp: 1715868828552,
        },
        {
            action: 'Send',
            success: true,
            timestamp: 1712186639729,
        },
        {
            action: 'Rewards',
            success: true,
            timestamp: 1715868828552,
        },
        {
            action: 'Receive',
            success: true,
            timestamp: 1712186639729,
        },
        {
            action: 'Transaction',
            success: true,
            timestamp: 1715868828552,
        },
        {
            action: 'Send',
            success: false,
            timestamp: 1712186639729,
        },
    ];

    const parentRef = React.useRef(null);
    const virtualizer = useVirtualizer({
        count: MOCK_ACTIVITIES.length,
        getScrollElement: () => parentRef.current,
        estimateSize: () => 100,
    });

    const virtualItems = virtualizer.getVirtualItems();

    return (
        <div
            className="flex flex-col items-center justify-center space-y-4 pt-12 h-full w-full"
        >
            <h1>Your Activity</h1>
            <div className="relative w-1/3 overflow-auto h-[50vh]" ref={parentRef}>
                {virtualItems.map((virtualItem) => {
                    const activity = MOCK_ACTIVITIES[virtualItem.index];
                    return (
                        <div
                            key={virtualItem.key}
                            className="absolute w-full pr-4 pb-4"
                            style={{
                                transform: `translateY(${virtualItem.start}px)`,
                                height: `${virtualItem.size}px`,
                            }}
                        >
                            <ActivityTile {...activity} />
                        </div>
                    );
                })}
            </div>
        </div>
    );
}

export default StakingDashboardPage;
