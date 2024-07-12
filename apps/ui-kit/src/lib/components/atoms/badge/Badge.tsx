// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { BadgeType } from './badge.enums';
import { BACKGROUND_COLORS, BADGE_TEXT_CLASS, BORDER_COLORS, TEXT_COLORS } from './badge.classes';

interface BadgeProps {
    /**
     * The type of the badge.
     */
    type?: BadgeType;
    /**
     * The label of the badge.
     */
    label: string;
    /**
     * The icon of the badge.
     */
    icon?: React.ReactNode;
    /**
     * The badge is disabled or not.
     */
    disabled?: boolean;
}

export function Badge({
    type: variant = BadgeType.Outlined,
    label,
    icon,
    disabled,
}: BadgeProps): React.JSX.Element {
    const backgroundClasses = BACKGROUND_COLORS[variant];
    const textClasses = TEXT_COLORS[variant];
    const borderClasses = BORDER_COLORS[variant];

    return (
        <div
            className={cx(
                'inline-flex items-center space-x-2 rounded-full border px-xs py-xxs disabled:opacity-30',
                backgroundClasses,
                borderClasses,
                {
                    'opacity-30': disabled,
                    'pr-sm': !!icon,
                },
            )}
        >
            {icon && <span className={cx(textClasses)}>{icon}</span>}
            <span className={cx(BADGE_TEXT_CLASS, textClasses)}>{label}</span>
        </div>
    );
}
