// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';

interface SecondaryTextProps {
    /**
     * The children to render.
     */
    children: React.ReactNode;
    /**
     * Should the text have error styles.
     */
    hasErrorStyles?: boolean;
}

export function SecondaryText({ children, hasErrorStyles }: SecondaryTextProps) {
    const ERROR_STYLES = 'group-[.errored]:secondary-text-error-color';
    return (
        <p
            className={cx('secondary-text-color text-label-lg ', {
                [ERROR_STYLES]: hasErrorStyles,
            })}
        >
            {children}
        </p>
    );
}
