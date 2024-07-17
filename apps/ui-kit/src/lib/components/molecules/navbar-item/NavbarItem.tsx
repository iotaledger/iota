// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import {
    BADGE_WITH_TEXT,
    BADGE_WITHOUT_TEXT,
    PADDING_WITH_TEXT,
    PADDING_WITHOUT_TEXT,
    SELECTED_BACKGROUND,
    SELECTED_ICON,
    SELECTED_TEXT,
    UNSELECTED_ICON,
    UNSELECTED_TEXT,
} from './navbarItem.classes';
import { Badge, BadgeType } from '../../atoms';

interface NavbarItemProps {
    /**
     * The icon of the navbar item.
     */
    icon: React.ReactNode;
    /**
     * The text of the navbar item.
     */
    text?: string;
    /**
     * Indicates if the navbar item is selected.
     */
    isSelected?: boolean;
    /**
     * Indicates if the navbar item has a badge.
     */
    hasBadge?: boolean;
    /**
     * The onClick event of the navbar item.
     */
    onClick?: (e: React.MouseEvent<HTMLDivElement>) => void;
}

export function NavbarItem({
    icon,
    text,
    isSelected,
    hasBadge,
    onClick,
}: NavbarItemProps): React.JSX.Element {
    const fillClasses = isSelected ? SELECTED_ICON : UNSELECTED_ICON;
    const paddingClasses = text ? PADDING_WITH_TEXT : PADDING_WITHOUT_TEXT;
    const backgroundColors = isSelected && SELECTED_BACKGROUND;
    const badgePositionClasses = text ? BADGE_WITH_TEXT : BADGE_WITHOUT_TEXT;
    const textClasses = isSelected ? SELECTED_TEXT : UNSELECTED_TEXT;
    return (
        <div
            onClick={onClick}
            className="inline-flex cursor-pointer flex-col items-center justify-center space-y-1"
        >
            <div
                className={cx(
                    'state-layer relative inline-flex rounded-full',
                    paddingClasses,
                    backgroundColors,
                )}
            >
                {React.cloneElement(icon as React.ReactElement, {
                    className: cx('w-6 h-6', fillClasses),
                })}
                {hasBadge && (
                    <div className={cx('absolute', badgePositionClasses)}>
                        <Badge type={BadgeType.PrimarySolid} />
                    </div>
                )}
            </div>
            {text && <span className={cx('text-center text-label-md', textClasses)}>{text}</span>}
        </div>
    );
}
