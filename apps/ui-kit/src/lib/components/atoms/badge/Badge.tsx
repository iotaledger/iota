// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { BadgeVariant } from './badge.enums';
import { BACKGROUND_COLORS, BADGE_TEXT_CLASS, OUTLINED_BORDER, TEXT_COLORS } from './badge.classes';

interface BadgeProps {
    /**
     * The variant of the badge.
     */
    variant?: BadgeVariant;
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
    variant = BadgeVariant.Outlined,
    label,
    icon,
    disabled = false,
}: BadgeProps): React.JSX.Element {
    const backgroundClasses = BACKGROUND_COLORS[variant];
    const textClasses = TEXT_COLORS[variant];
    const borderClasses = variant === BadgeVariant.Outlined ? OUTLINED_BORDER : '';

    return (
        <div
            className={cx(
                'flex items-center space-x-2 rounded-full px-xs py-xxs',
                backgroundClasses,
                borderClasses,
                {
                    'opacity-30': disabled,
                },
            )}
        >
            {icon && <span className={cx(textClasses)}>{icon}</span>}
            <span className={cx(BADGE_TEXT_CLASS, textClasses)}>{label}</span>
        </div>
    );
}
