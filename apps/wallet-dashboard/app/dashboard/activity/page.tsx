// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import ActivityTile from '@/components/ActivityTile';
import { useVirtualizer } from '@tanstack/react-virtual';
import React, { useMemo } from 'react';
import { useCurrentAccount } from '@mysten/dapp-kit';
import { useQueryTransactionsByAddress } from '@/hooks/useQueryTransactionsByAddress';
import { getTransactionActivity } from '@/lib/utils/activity';

function ActivityPage(): JSX.Element {
    const containerRef = React.useRef(null);
    const currentAccount = useCurrentAccount();
    const { data: txs, error } = useQueryTransactionsByAddress(currentAccount?.address);

    const mapped = useMemo(() => {
        return txs?.map((tx) => getTransactionActivity(tx, currentAccount?.address)) || [];
    }, [currentAccount?.address, txs]);

    const virtualizer = useVirtualizer({
        count: mapped?.length || 0,
        getScrollElement: () => containerRef.current,
        estimateSize: () => 100,
    });

    if (error) {
        return <div>{(error as Error)?.message}</div>;
    }

    const virtualItems = virtualizer.getVirtualItems();

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4 pt-12">
            <h1>Your Activity</h1>
            <div className="relative h-[50vh] w-1/3 overflow-auto" ref={containerRef}>
                <div
                    style={{
                        height: `${virtualizer.getTotalSize()}px`,
                        width: '100%',
                        position: 'relative',
                    }}
                >
                    {mapped &&
                        virtualItems.map((virtualItem) => {
                            const activity = mapped[virtualItem.index];
                            return (
                                <div
                                    key={virtualItem.key}
                                    className="absolute w-full pb-4 pr-4"
                                    style={{
                                        position: 'absolute',
                                        top: 0,
                                        left: 0,
                                        width: '100%',
                                        height: `${virtualItem.size}px`,
                                        transform: `translateY(${virtualItem.start}px)`,
                                    }}
                                >
                                    {activity && <ActivityTile activity={activity} />}
                                </div>
                            );
                        })}
                </div>
            </div>
        </div>
    );
}

export default ActivityPage;
