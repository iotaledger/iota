// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React, { ReactNode, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';

interface VirtualListProps<T> {
    items: T[];
    hasNextPage?: boolean;
    isFetchingNextPage?: boolean;
    fetchNextPage?: () => void;
    estimateSize: (index: number) => number;
    render: (item: T, index: number) => ReactNode;
    onClick?: (item: T) => void;
    heightClassName?: string;
    overflowClassName?: string;
}

function VirtualList<T>({
    items,
    hasNextPage = false,
    isFetchingNextPage = false,
    fetchNextPage,
    estimateSize,
    render,
    onClick,
    heightClassName = 'h-fit',
    overflowClassName,
}: VirtualListProps<T>): JSX.Element {
    const containerRef = React.useRef<HTMLDivElement | null>(null);
    const virtualizer = useVirtualizer({
        // Render one more item if there is still pages to be fetched
        count: hasNextPage ? items.length + 1 : items.length,
        getScrollElement: () => containerRef.current,
        estimateSize: (index) => {
            if (index > items.length - 1 && hasNextPage) {
                return 20;
            } else {
                return estimateSize(index);
            }
        },
    });

    const virtualItems = virtualizer.getVirtualItems();

    useEffect(() => {
        const [lastItem] = [...virtualizer.getVirtualItems()].reverse();
        if (!lastItem || !fetchNextPage) {
            return;
        }

        // Fetch the next page if the last rendered item is the one we added as extra, and there is still more pages to fetch
        if (lastItem.index >= items.length - 1 && hasNextPage && !isFetchingNextPage) {
            fetchNextPage();
        }
    }, [hasNextPage, fetchNextPage, items.length, isFetchingNextPage, virtualizer, virtualItems]);

    return (
        <div
            className={clsx('relative w-full', heightClassName, overflowClassName)}
            ref={containerRef}
        >
            <div
                style={{
                    height: `${virtualizer.getTotalSize()}px`,
                    width: '100%',
                    position: 'relative',
                }}
            >
                {virtualItems.map((virtualItem) => {
                    const item = items[virtualItem.index];
                    return (
                        <div
                            key={virtualItem.key}
                            className={`absolute w-full  ${onClick ? 'cursor-pointer' : ''}`}
                            style={{
                                position: 'absolute',
                                top: 0,
                                left: 0,
                                width: '100%',
                                height: `${virtualItem.size}px`,
                                transform: `translateY(${virtualItem.start}px)`,
                            }}
                            onClick={() => onClick && onClick(item)}
                        >
                            {virtualItem.index > items.length - 1
                                ? hasNextPage
                                    ? 'Loading more...'
                                    : 'Nothing more to load'
                                : render(item, virtualItem.index)}
                        </div>
                    );
                })}
            </div>
        </div>
    );
}

export default VirtualList;
