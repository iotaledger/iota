// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { BACKGROUND_COLORS } from './skeleton.constants';

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
    const bgColor = hasSecondaryColors ? BACKGROUND_COLORS.secondary : BACKGROUND_COLORS.primary;
    return (
        <div
            className={cx(
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
