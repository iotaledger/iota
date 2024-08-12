// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ChevronDown16, ChevronRight16 } from '@iota/icons';
import clsx from 'clsx';
import { useState, type ReactNode } from 'react';

import { Text } from '_app/shared/text';

interface SummaryCardProps {
    header: ReactNode;
    children: ReactNode;
    badge?: ReactNode;
    initialExpanded?: boolean;
}

export function SummaryCard({
    children,
    header,
    badge,
    initialExpanded = false,
}: SummaryCardProps) {
    const [expanded, setExpanded] = useState(initialExpanded);

    return (
        <div
            className={clsx(
                'overflow-hidden rounded-2xl border border-solid',
                expanded ? 'border-gray-45' : 'border-gray-40',
            )}
        >
            <button
                onClick={() => setExpanded((expanded) => !expanded)}
                className="bg-gray-40 relative flex w-full cursor-pointer items-center gap-1.5 border-none px-4 py-2 text-left"
            >
                <div className="flex-1">
                    <Text variant="captionSmall" weight="semibold" color="steel-darker">
                        {header}
                    </Text>
                </div>

                {badge}

                <div className="text-steel flex items-center justify-center">
                    {expanded ? <ChevronDown16 /> : <ChevronRight16 />}
                </div>
            </button>
            {expanded && <div className="px-4 py-3">{children}</div>}
        </div>
    );
}
