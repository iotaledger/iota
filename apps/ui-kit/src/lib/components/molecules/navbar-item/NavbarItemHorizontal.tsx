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
import { NavbarItemProps } from './NavbarItem';

export function NavbarItemHorizontal({
    icon,
    text,
    isSelected,
    hasBadge,
    onClick,
}: Omit<NavbarItemProps, 'type'>): React.JSX.Element {
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
                <div className={cx('inline-flex [&_svg]:h-6 [&_svg]:w-6', fillClasses)}>{icon}</div>
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
