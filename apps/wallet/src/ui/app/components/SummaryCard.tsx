// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from 'react';
import { Title, TitleSize } from '@iota/apps-ui-kit';

export interface SummaryCardProps {
    header?: string;
    body: ReactNode;
    footer?: ReactNode;
}

export function SummaryCard({ body, header, footer }: SummaryCardProps) {
    return (
        <div className="flex w-full flex-col flex-nowrap gap-xs rounded-xl bg-neutral-96 pb-md">
            {header ? (
                <div className="flex h-[56px] items-center">
                    <Title title={header} size={TitleSize.Small} />
                </div>
            ) : null}

            {body && <div className="px-md">{body}</div>}
            {footer ? (
                <div className="border-gray-40 border-x-0 border-b-0 border-t border-solid p-4 pt-3">
                    {footer}
                </div>
            ) : null}
        </div>
    );
}
