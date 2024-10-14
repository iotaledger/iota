// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { Loader } from '@iota/ui-icons';

export interface LoadingIndicatorProps {
    /**
     * The color of the loading indicator.
     */
    color?: string;
    /**
     * The size of the loading indicator.
     */
    size?: string;
    /**
     * The text to display next to the loading indicator.
     */
    text?: string;
}

export function LoadingIndicator({
    color = 'text-primary-30',
    size = 'w-5 h-5',
    text,
}: LoadingIndicatorProps): React.JSX.Element {
    return (
        <div className="flex items-center justify-center gap-xs">
            <Loader className={cx('animate-spin', color, size)} />
            {text && <span className={cx('text-body-sm', color)}>{text}</span>}
        </div>
    );
}
