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

export function Placeholder({ width = 'w-full', height = 'h-4' }: PlaceholderProps) {
    return (
        <div
            className={cx(
                'animate-pulse rounded-md bg-gradient-to-r from-shader-primary-light-8 bg-[length:1000px_100%] dark:from-shader-primary-dark-8',
                width,
                height,
            )}
        />
    );
}
