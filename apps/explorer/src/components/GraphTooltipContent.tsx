// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTooltipPosition } from '@visx/tooltip';
import clsx from 'clsx';

export function GraphTooltipContainer({ children }: React.PropsWithChildren): JSX.Element {
    const { isFlippedHorizontally } = useTooltipPosition();
    return (
        <div
            className={clsx(
                'border-neutral-70 dark:border-neutral-30 dark:bg-neutral-10/90 dark:shadow-neutral-0/20 w-fit -translate-y-[calc(100%-10px)] rounded-md border border-solid bg-neutral-100/90 p-xs shadow-xl',
                isFlippedHorizontally
                    ? '-translate-x-[1px] rounded-bl-none'
                    : 'translate-x-[1px] rounded-br-none',
            )}
        >
            {children}
        </div>
    );
}

interface GraphTooltipContentProps {
    title: string;
    overline: string;
    subtitle: string;
}
export function GraphTooltipContent({ title, overline, subtitle }: GraphTooltipContentProps) {
    return (
        <div className="flex flex-col gap-xxxs">
            <span className="text-neutral-40 dark:text-neutral-60 text-body-sm">{overline}</span>
            <span className="text-neutral-12 dark:text-neutral-98 text-label-lg">{title}</span>
            <span className="text-neutral-40 dark:text-neutral-60 text-body-sm">{subtitle}</span>
        </div>
    );
}
