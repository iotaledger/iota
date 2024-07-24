// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import {
    BACKGROUND_COLORS,
    BACKGROUND_COLORS_SELECTED,
    TEXT_COLORS,
    TEXT_COLORS_SELECTED,
    UNDERLINED_BACKGROUND_SELECTED,
} from './button-segment.classes';
import cx from 'classnames';
import { ButtonSegmentType } from './button-segment.enums';

interface ButtonSegmentProps {
    /**
     * The label of the button.
     */
    label: string;
    /**
     The icon of the button
     */
    icon?: React.ReactNode;
    /**
     The selected flag of the button
     */
    selected?: boolean;
    /**
     * The button is disabled or not.
     */
    disabled?: boolean;
    /**
     * The onClick event of the button.
     */
    onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * The type of the button.
     */
    type?: ButtonSegmentType;
}

export function ButtonSegment({
    icon,
    label,
    selected,
    disabled,
    onClick,
    type = ButtonSegmentType.Rounded,
}: ButtonSegmentProps): React.JSX.Element {
    const isUnderlined = type === ButtonSegmentType.Underlined;

    const backgroundColors = selected
        ? isUnderlined
            ? UNDERLINED_BACKGROUND_SELECTED
            : BACKGROUND_COLORS_SELECTED
        : BACKGROUND_COLORS;

    const textColors = selected ? TEXT_COLORS_SELECTED : TEXT_COLORS;
    const padding = isUnderlined ? 'px-lg py-md' : 'px-sm py-[6px]';
    const borderRadius = isUnderlined ? '' : 'rounded-full';
    return (
        <button
            onClick={onClick}
            className={cx(
                'enabled:state-layer relative flex items-center disabled:opacity-40',
                backgroundColors,
                textColors,
                padding,
                borderRadius,
                {
                    'pl-xs': !!icon && !isUnderlined,
                },
            )}
            disabled={disabled}
        >
            <div className={cx('flex flex-row items-center justify-center gap-2 text-body-md')}>
                {icon && <span>{icon}</span>}
                <span className="font-inter">{label}</span>
            </div>
        </button>
    );
}
