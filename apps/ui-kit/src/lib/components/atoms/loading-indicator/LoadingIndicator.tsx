// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
    /**
     * The 'data-testid' attribute value (used in e2e tests)
     */
    testId?: string;
}

export function LoadingIndicator({
    color = 'text-primary-30 dark:text-primary-80',
    size = 'w-5 h-5',
    text,
    testId,
}: LoadingIndicatorProps): React.JSX.Element {
    return (
        <div className="flex items-center justify-center gap-xs">
            <Loader className={cx('animate-spin', color, size)} data-testid={testId} />
            {text && <span className={cx('text-body-sm', color)}>{text}</span>}
        </div>
    );
}
