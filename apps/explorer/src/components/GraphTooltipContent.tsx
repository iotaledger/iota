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
                'w-fit -translate-y-[calc(100%-10px)] rounded-md border border-solid border-iota-neutral-70 bg-iota-neutral-100/90 p-xs shadow-xl dark:border-iota-neutral-30 dark:bg-iota-neutral-10/90 dark:shadow-iota-neutral-0/20',
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
            <span className="text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                {overline}
            </span>
            <span className="text-label-lg text-iota-neutral-12 dark:text-iota-neutral-98">
                {title}
            </span>
            <span className="text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                {subtitle}
            </span>
        </div>
    );
}
