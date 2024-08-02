// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { PanelSize, PanelTitleSize } from './panel.enums';
import {
    PADDING_BOTTOM_TITLE,
    PADDING_TOP_CHILDREN_WITH_TITLE,
    PADDING_TOP_WITH_TITLE,
} from './panel.classes';
import { Badge, BadgeType } from '../badge';

interface PanelProps {
    /**
     * The title of the panel.
     */
    title?: string;
    /**
     * The size of the title.
     */
    titleSize?: PanelTitleSize;
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
    /**
     * The size of the panel.
     */
    size?: PanelSize;
}

export function Panel({
    title,
    titleSize = PanelTitleSize.Large,
    children,
    badgeType,
    badgeText,
    hasBorder,
    size = PanelSize.Medium,
}: React.PropsWithChildren<PanelProps>): React.JSX.Element {
    const titleSizeClass = titleSize === PanelTitleSize.Medium ? 'text-title-md' : 'text-title-lg';
    const borderClass = hasBorder
        ? 'border border-shader-neutral-light-8 dark:border-shader-neutral-dark-8'
        : 'border border-transparent';
    const panelPaddingClass = size === PanelSize.Medium ? 'px-md--rs pb-md--rs' : 'px-md pb-md';
    const PADDING_TOP_WITHOUT_TITLE = size === PanelSize.Medium ? 'pt-md--rs' : 'pt-md';
    const paddingTopClass = title ? PADDING_TOP_WITH_TITLE[size] : PADDING_TOP_WITHOUT_TITLE;
    const paddingTopChildren = title ? PADDING_TOP_CHILDREN_WITH_TITLE[size] : 'pt-0';
    const paddingBottomTitleClass = PADDING_BOTTOM_TITLE[size];
    return (
        <div
            className={cx(
                'flex flex-col rounded-xl bg-neutral-100 px-md dark:bg-neutral-10',
                borderClass,
                panelPaddingClass,
                paddingTopClass,
            )}
        >
            {title && (
                <div className={cx('flex items-center gap-0.5', paddingBottomTitleClass)}>
                    <span
                        className={cx(
                            'font-inter text-neutral-10 dark:text-neutral-92',
                            titleSizeClass,
                        )}
                    >
                        {title}
                    </span>
                    {badgeType && badgeText && <Badge type={badgeType} label={badgeText} />}
                </div>
            )}
            <div className={cx(paddingTopChildren)}>{children}</div>
        </div>
    );
}
