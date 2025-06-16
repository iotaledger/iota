// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { ReactNode, useEffect, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import { LoadingIndicator } from '@iota/apps-ui-kit';

interface VirtualListProps<T> {
    items: T[];
    hasNextPage?: boolean;
    isFetchingNextPage?: boolean;
    fetchNextPage?: () => void;
    estimateSize: (index: number) => number;
    render: (item: T, index: number) => ReactNode;
    onClick?: (item: T) => void;
    heightClassName?: string;
    getItemKey?: (item: T) => string | number;
}

export function VirtualList<T>({
    items,
    hasNextPage = false,
    isFetchingNextPage = false,
    fetchNextPage,
    estimateSize,
    render,
    onClick,
    heightClassName = 'h-fit',
    getItemKey,
}: VirtualListProps<T>): JSX.Element {
    const containerRef = useRef<HTMLDivElement>(null);
    const measuredHeights = useRef<Map<number, number>>(new Map());

    const virtualizer = useVirtualizer({
        // Render an extra item if there is still pages to be fetched
        count: hasNextPage ? items.length + 1 : items.length,
        getScrollElement: () => containerRef.current,
        estimateSize: (index) => {
            // Check if the item is already measured and return its height
            if (measuredHeights.current.has(index)) {
                return measuredHeights.current.get(index)!;
            }

            if (index > items.length - 1 && hasNextPage) {
                return 20;
            }
            return estimateSize(index);
        },
        overscan: 3, // Number of items to render outside the viewport to improve performance and avoid flickering
    });

    const virtualItems = virtualizer.getVirtualItems();

    useEffect(() => {
        const [lastItem] = [...virtualItems].reverse();
        if (!lastItem || !fetchNextPage) {
            return;
        }

        // Fetch the next page if the last rendered item is the one we added as extra, and there is still more pages to fetch
        if (lastItem.index >= items.length - 1 && hasNextPage && !isFetchingNextPage) {
            fetchNextPage();
        }
    }, [hasNextPage, fetchNextPage, items.length, isFetchingNextPage, virtualItems]);

    return (
        <div
            ref={containerRef}
            className={clsx('relative w-full overflow-y-auto', heightClassName)}
        >
            <div
                style={{
                    height: `${virtualizer.getTotalSize()}px`,
                    width: '100%',
                    position: 'relative',
                }}
            >
                {virtualItems.map((virtualItem) => {
                    // Last item is reserved to show a "Loading..." if there are still more pages to be fetched
                    const isExtraItem = virtualItem.index > items.length - 1;
                    const item = items[virtualItem.index];
                    const key = !isExtraItem && getItemKey ? getItemKey(item) : virtualItem.key;
                    return (
                        <div
                            key={key}
                            className={`absolute left-0 top-0 w-full ${onClick ? 'cursor-pointer' : ''}`}
                            style={{
                                transform: `translateY(${virtualItem.start}px)`,
                            }}
                            ref={(el) => {
                                if (el) {
                                    // Measure the height of the element and store it in the map
                                    const height = el.getBoundingClientRect().height;
                                    measuredHeights.current.set(virtualItem.index, height);
                                    virtualizer.measureElement(el);
                                }
                            }}
                            data-index={virtualItem.index}
                            onClick={() => !isExtraItem && onClick?.(item)}
                        >
                            {isExtraItem && hasNextPage ? (
                                <LoadingIndicator text="Loading more..." />
                            ) : (
                                render(item, virtualItem.index)
                            )}
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
