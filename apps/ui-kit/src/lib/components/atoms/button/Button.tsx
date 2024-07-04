// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { ButtonSize, ButtonType } from './button.enums';
import {
    PADDINGS,
    PADDINGS_ONLY_ICON,
    BACKGROUND_COLORS,
    TEXT_COLORS,
    TEXT_CLASSES,
    TEXT_COLOR_DISABLED,
    DISABLED_BACKGROUND_COLORS,
} from './button.classes';
import cx from 'classnames';

interface ButtonProps {
    /**
     * The size of the button.
     */
    size?: ButtonSize;
    /**
     * The type of the button
     */
    type?: ButtonType;
    /**
     * The text of the button.
     */
    text?: string;
    /**
     The icon of the button
     */
    icon?: React.ReactNode;
    /**
     * The button is disabled or not.
     */
    disabled?: boolean;
    /**
     *
     */
    onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export function Button({
    icon,
    text,
    disabled,
    onClick,
    size = ButtonSize.Medium,
    type = ButtonType.Primary,
}: ButtonProps): React.JSX.Element {
    const paddingClasses = icon && !text ? PADDINGS_ONLY_ICON[size] : PADDINGS[size];
    const backgroundColors = disabled ? DISABLED_BACKGROUND_COLORS[type] : BACKGROUND_COLORS[type];
    const textColors = disabled ? TEXT_COLOR_DISABLED[type] : TEXT_COLORS[type];
    const textSizes = TEXT_CLASSES[size];
    return (
        <button
            onClick={onClick}
            className={cx(
                'state-layer relative flex rounded-full disabled:opacity-40',
                paddingClasses,
                backgroundColors,
            )}
            disabled={disabled}
        >
            <div className="flex flex-row items-center justify-center gap-2">
                {icon && <span className={cx(textColors)}>{icon}</span>}
                {text && <span className={cx('font-inter', textColors, textSizes)}>{text}</span>}
            </div>
        </button>
    );
}
