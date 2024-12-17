// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';

interface SkeletonLoaderProps {
    /**
     * Width class for the skeleton div.
     */
    widthClass?: string;
    /**
     * Height class for the skeleton div.
     */
    heightClass?: string;
    /**
     * If true, the skeleton will use darker neutral colors.
     */
    hasSecondaryColors?: boolean;
    /**
     * Whether the class `rounded-full` should be applied. Defaults to true.
     */
    isRounded?: boolean;
}

export function Skeleton({
    children,
    widthClass = 'w-full',
    heightClass = 'h-3',
    hasSecondaryColors,
    isRounded = true,
}: React.PropsWithChildren<SkeletonLoaderProps>): React.JSX.Element {
    return (
        <div
            className={cx(
                'animate-pulse rounded-full',
                widthClass,
                heightClass,
                isRounded && 'rounded-full',
                hasSecondaryColors
                    ? 'bg-neutral-80 dark:bg-neutral-10'
                    : 'bg-neutral-90 dark:bg-neutral-12',
            )}
        >
            {children}
        </div>
    );
}
