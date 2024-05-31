// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { HARDCODED_NON_VISUAL_ASSETS } from '@/lib/mocks';
import React from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Box } from '@/components/index';

function EverythingElsePage(): JSX.Element {
    const containerRef = React.useRef(null);
    const virtualizer = useVirtualizer({
        count: HARDCODED_NON_VISUAL_ASSETS.length,
        getScrollElement: () => containerRef.current,
        estimateSize: () => 130,
    });

    const virtualItems = virtualizer.getVirtualItems();
    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>EVERYTHING ELSE</h1>
            <div className="relative h-[50vh] w-2/3 overflow-auto" ref={containerRef}>
                <div
                    style={{
                        height: `${virtualizer.getTotalSize()}px`,
                        width: '100%',
                        position: 'relative',
                    }}
                >
                    {virtualItems.map((virtualItem) => {
                        const asset = HARDCODED_NON_VISUAL_ASSETS[virtualItem.index];
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
                                <Box>
                                    <div className="flex gap-2">
                                        <div>
                                            <p>Type: {asset.type}</p>
                                            <p>Digest: {asset.digest}</p>
                                            <p>Object ID: {asset.objectId}</p>
                                        </div>
                                    </div>
                                </Box>
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
}

export default EverythingElsePage;
