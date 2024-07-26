// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { TitleSize } from './box.enums';

interface BoxProps {
    /**
     * The title of the box.
     */
    title?: string;
    /**
     * The size of the title.
     */
    size?: TitleSize;
}

export function Box({
    title,
    size = TitleSize.Medium,
    children,
}: React.PropsWithChildren<BoxProps>): React.JSX.Element {
    const titleSize = size === TitleSize.Medium ? 'text-title-md' : 'text-title-lg';
    return (
        <div className="flex w-full flex-col rounded-xl border border-shader-neutral-light-8 p-md dark:border-shader-neutral-dark-8">
            {title && (
                <span
                    className={cx(
                        'font-inter text-title-md text-neutral-10 dark:text-neutral-92',
                        titleSize,
                    )}
                >
                    {title}
                </span>
            )}
            {children}
        </div>
    );
}
