// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';

interface PanelProps {
    /**
     * Show or hide border around the panel.
     */
    hasBorder?: boolean;
    /**
     * Background color of the panel.
     */
    bgColor?: string;
}

export function Panel({
    children,
    hasBorder,
    bgColor = 'bg-neutral-100 dark:bg-neutral-6',
}: React.PropsWithChildren<PanelProps>): React.JSX.Element {
    const borderClass = hasBorder
        ? 'border border-shader-neutral-light-8 dark:border-shader-neutral-dark-8'
        : 'border border-transparent';
    return (
        <div className={cx('flex w-full flex-col rounded-xl', bgColor, borderClass)}>
            {children}
        </div>
    );
}
