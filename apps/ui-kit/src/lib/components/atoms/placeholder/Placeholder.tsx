// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';

interface PlaceholderProps {
    /**
     * The width of the placeholder.
     */
    width?: string;
    /**
     * The height of the placeholder.
     */
    height?: string;
}

export function Placeholder({ width, height }: PlaceholderProps) {
    const widthClass = width ? `${width}` : 'w-full';
    const heightClass = height ? `${height}` : 'h-4';

    return (
        <div
            className={cx(
                'animate-pulse rounded-md bg-gradient-to-r from-shader-primary-light-8 bg-[length:1000px_100%] dark:from-shader-primary-dark-8',
                widthClass,
                heightClass,
            )}
        />
    );
}
