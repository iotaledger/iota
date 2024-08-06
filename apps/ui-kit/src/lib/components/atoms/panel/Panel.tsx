// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';

interface PanelProps {
    /**
     * Show or hide border around the panel.
     */
    hasBorder?: boolean;
}

export function Panel({
    children,
    hasBorder,
}: React.PropsWithChildren<PanelProps>): React.JSX.Element {
    const borderClass = hasBorder
        ? 'border border-shader-neutral-light-8 dark:border-shader-neutral-dark-8'
        : 'border border-transparent';
    return (
        <div
            className={cx(
                'flex flex-col rounded-xl bg-neutral-100 dark:bg-neutral-10',
                borderClass,
            )}
        >
            {children}
        </div>
    );
}
