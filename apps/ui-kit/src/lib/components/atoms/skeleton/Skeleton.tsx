// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import React from 'react';

interface SkeletonLoaderProps extends Pick<React.HTMLAttributes<HTMLDivElement>, 'className'> {
    /**
     * If true, the skeleton will use darker neutral colors.
     */
    hasSecondaryColors?: boolean;
}

export function Skeleton({
    children,
    className,
    hasSecondaryColors,
}: React.PropsWithChildren<SkeletonLoaderProps>): React.JSX.Element {
    return (
        <div
            className={cx(
                'h-3 w-full animate-pulse rounded-full',
                hasSecondaryColors ? 'skeleton-secondary-bg' : 'skeleton-bg',
                className,
            )}
        >
            {children}
        </div>
    );
}
