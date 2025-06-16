// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import type { BadgeType } from './badge.enums';
import { BACKGROUND_COLORS, BADGE_TEXT_CLASS, BORDER_COLORS, TEXT_COLORS } from './badge.classes';

interface BadgeProps {
    /**
     * The type of the badge.
     */
    type: BadgeType;
    /**
     * The label of the badge.
     */
    label?: string;
}

export function Badge({ type, label }: BadgeProps): React.JSX.Element {
    const backgroundClasses = BACKGROUND_COLORS[type];
    const textClasses = TEXT_COLORS[type];
    const borderClasses = BORDER_COLORS[type];
    const labelClasses = label ? 'px-xs py-xxs' : 'h-1.5 w-1.5';

    return (
        <div
            className={cx(
                'inline-flex items-center space-x-2 rounded-full border disabled:opacity-30',
                backgroundClasses,
                borderClasses,
                labelClasses,
            )}
        >
            <span className={cx(BADGE_TEXT_CLASS, textClasses)}>{label}</span>
        </div>
    );
}
