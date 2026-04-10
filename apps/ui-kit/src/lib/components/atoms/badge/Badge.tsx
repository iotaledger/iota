// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { BadgeSize } from './badge.enums';
import type { BadgeType } from './badge.enums';
import { BACKGROUND_COLORS, BORDER_COLORS, TEXT_COLORS } from './badge.classes';

interface BadgeProps {
    /**
     * The type of the badge.
     */
    type: BadgeType;
    /**
     * The label of the badge.
     */
    label?: string;
    /**
     * The size of the badge
     */
    size?: BadgeSize;
}

export function Badge({ type, label, size = BadgeSize.Medium }: BadgeProps): React.JSX.Element {
    const backgroundClasses = BACKGROUND_COLORS[type];
    const textClasses = TEXT_COLORS[type];
    const isSmall = size === BadgeSize.Small;
    const labelClasses = label ? 'px-xs py-xxs' : 'h-1.5 w-1.5';
    const textSizeClass = isSmall ? 'text-label-sm' : 'text-label-md';

    return (
        <div
            className={cx(
                'inline-flex items-center space-x-2 rounded-full disabled:opacity-30',
                { border: !isSmall },
                backgroundClasses,
                { [BORDER_COLORS[type]]: !isSmall },
                labelClasses,
            )}
        >
            <span className={cx(textSizeClass, textClasses)}>{label}</span>
        </div>
    );
}
