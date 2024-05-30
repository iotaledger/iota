// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

interface VirtualListProps<T> {
    items: T[];
    estimateSize: () => number;
    renderVirtualItem: (item: T) => React.ReactNode;
}

function VirtualList<T>({
    items,
    estimateSize,
    renderVirtualItem,
}: VirtualListProps<T>): JSX.Element {
    const containerRef = React.useRef<HTMLDivElement | null>(null);
    const virtualizer = useVirtualizer({
        count: items.length,
        getScrollElement: () => containerRef.current,
        estimateSize,
    });

    const virtualItems = virtualizer.getVirtualItems();

    return (
        <div className="relative h-[50vh] w-full overflow-auto" ref={containerRef}>
            <div
                style={{
                    height: `${virtualizer.getTotalSize()}px`,
                    width: '100%',
                    position: 'relative',
                }}
            >
                {virtualItems.map((virtualItem) => (
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
                        {renderVirtualItem(items[virtualItem.index])}
                    </div>
                ))}
            </div>
        </div>
    );
}

export default VirtualList;
