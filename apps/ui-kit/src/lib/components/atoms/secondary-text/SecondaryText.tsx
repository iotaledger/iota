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
    const ERROR_STYLES = 'group-[.errored]:text-error-30 dark:group-[.errored]:text-error-80';
    return (
        <p
            className={cx('text-label-lg text-neutral-40  dark:text-neutral-60 ', {
                [ERROR_STYLES]: hasErrorStyles,
            })}
        >
            {children}
        </p>
    );
}
