// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';
import type { ReactNode } from 'react';

import { Text } from '../shared/text';

export interface SummaryCardProps {
    header?: string;
    body: ReactNode;
    footer?: ReactNode;
    minimalPadding?: boolean;
    showDivider?: boolean;
    noBorder?: boolean;
    boxShadow?: boolean;
}

export function SummaryCard({
    body,
    header,
    footer,
    minimalPadding,
    showDivider = false,
    noBorder = false,
    boxShadow = false,
}: SummaryCardProps) {
    return (
        <div
            className={clsx(
                { 'border-gray-45 border border-solid': !noBorder, 'shadow-card-soft': boxShadow },
                'flex w-full flex-col flex-nowrap rounded-2xl bg-white',
            )}
        >
            {header ? (
                <div className="bg-gray-40 flex flex-row flex-nowrap items-center justify-center rounded-t-2xl px-3.75 py-2.5 uppercase">
                    <Text variant="captionSmall" weight="bold" color="steel-darker" truncate>
                        {header}
                    </Text>
                </div>
            ) : null}
            <div
                className={clsx(
                    'flex flex-1 flex-col flex-nowrap items-stretch overflow-y-auto px-4',
                    minimalPadding ? 'py-2' : 'py-4',
                    showDivider ? 'divide-gray-40 divide-x-0 divide-y divide-solid' : '',
                )}
            >
                {body}
            </div>
            {footer ? (
                <div className="border-gray-40 border-x-0 border-b-0 border-t border-solid p-4 pt-3">
                    {footer}
                </div>
            ) : null}
        </div>
    );
}
