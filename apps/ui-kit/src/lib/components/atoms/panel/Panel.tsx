// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { PanelTitleSize } from './panel.enums';
import { PADDING_BOTTOM_TITLE, PADDING_CHILDREN, PADDING_WITH_TITLE } from './panel.classes';
import { Badge, BadgeType } from '../badge';

interface PanelProps {
    /**
     * The title of the panel.
     */
    title?: string;
    /**
     * The size of the title.
     */
    size?: PanelTitleSize;
    /**
     * The type of the badge.
     */
    badgeType?: BadgeType;
    /**
     * The text of the badge.
     */
    badgeText?: string;
    /**
     * Show or hide border around the panel.
     */
    hasBorder?: boolean;
}

export function Panel({
    title,
    size = PanelTitleSize.Medium,
    children,
    badgeType,
    badgeText,
    hasBorder,
}: React.PropsWithChildren<PanelProps>): React.JSX.Element {
    const titleSize = size === PanelTitleSize.Medium ? 'text-title-md' : 'text-title-lg';
    const padding = title ? PADDING_WITH_TITLE[size] : 'py-md';
    const paddingBottom = PADDING_BOTTOM_TITLE[size];
    const paddingChildren = title ? PADDING_CHILDREN[size] : '';
    const border = hasBorder
        ? 'border border-shader-neutral-light-8 dark:border-shader-neutral-dark-8'
        : 'border border-transparent';
    return (
        <div
            className={cx(
                'flex w-full flex-col rounded-xl  bg-neutral-100 px-md dark:bg-neutral-10',
                padding,
                border,
            )}
        >
            {title && (
                <div className={cx('flex items-center gap-0.5', paddingBottom)}>
                    <span
                        className={cx('font-inter text-neutral-10 dark:text-neutral-92', titleSize)}
                    >
                        {title}
                    </span>
                    {badgeType && badgeText && <Badge type={badgeType} label={badgeText} />}
                </div>
            )}
            <div className={cx(paddingChildren)}>{children}</div>
        </div>
    );
}
