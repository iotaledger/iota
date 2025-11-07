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
    bgColor = 'panel-bg',
}: React.PropsWithChildren<PanelProps>): React.JSX.Element {
    const borderClass = hasBorder ? 'border panel-border-color' : 'border border-transparent';
    return (
        <div className={cx('flex w-full flex-col rounded-xl', bgColor, borderClass)}>
            {children}
        </div>
    );
}
