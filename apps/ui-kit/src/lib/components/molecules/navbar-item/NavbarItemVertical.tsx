// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import {
    SELECTED_BACKGROUND,
    SELECTED_ICON,
    SELECTED_TEXT,
    UNSELECTED_ICON,
    UNSELECTED_TEXT,
} from './navbarItem.classes';
import { Badge, BadgeType } from '../../atoms';
import { NavbarItemProps } from './NavbarItem';

export function NavbarItemVertical({
    icon,
    text,
    isSelected,
    hasBadge,
    onClick,
    isDisabled = false,
}: Omit<NavbarItemProps, 'type'>): React.JSX.Element {
    const fillClasses = isSelected ? SELECTED_ICON : UNSELECTED_ICON;
    const backgroundColors = isSelected && SELECTED_BACKGROUND;
    const textClasses = isSelected ? SELECTED_TEXT : UNSELECTED_TEXT;
    const disabledClasses = isDisabled
        ? 'cursor-not-allowed opacity-60'
        : 'state-layer cursor-pointer ';
    const handleClick = (e: React.MouseEvent<HTMLDivElement>) => {
        if (isDisabled) {
            e.preventDefault();
            return;
        }
        onClick?.(e);
    };
    return (
        <div
            onClick={handleClick}
            className={cx(
                'relative inline-flex w-full flex-row items-center justify-between rounded-full p-sm',
                backgroundColors,
                disabledClasses,
            )}
        >
            <div className="flex items-center gap-3">
                <div className={cx('inline-flex [&_svg]:h-6 [&_svg]:w-6', fillClasses)}>{icon}</div>
                {text && (
                    <span className={cx('text-center text-label-lg', textClasses)}>{text}</span>
                )}
            </div>
            {hasBadge && <Badge type={BadgeType.PrimarySolid} />}
        </div>
    );
}
