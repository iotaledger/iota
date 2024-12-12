// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';

interface SkeletonLoaderProps {
    widthClass?: string;
    heightClass?: string;
    hasSecondaryColors?: boolean;
    isRounded?: boolean;
}

const BACKGROUND_COLORS = {
    primary: 'bg-neutral-90 dark:bg-neutral-12',
    secondary: 'bg-neutral-80 dark:bg-neutral-10',
};

export function SkeletonLoader({
    children,
    widthClass = 'w-full',
    heightClass = 'h-3',
    hasSecondaryColors,
    isRounded = true,
}: React.PropsWithChildren<SkeletonLoaderProps>): React.JSX.Element {
    const bgColor = hasSecondaryColors ? BACKGROUND_COLORS.secondary : BACKGROUND_COLORS.primary;
    return (
        <div
            className={clsx(
                'animate-pulse rounded-full',
                widthClass,
                heightClass,
                bgColor,
                isRounded && 'rounded-full',
            )}
        >
            {children}
        </div>
    );
}
