// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { ButtonHtmlType, ButtonSize, ButtonType } from './button.enums';
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

export interface ButtonProps {
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
     * The onClick event of the button.
     */
    onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * Whether the button should have the full width.
     */
    fullWidth?: boolean;
    /**
     * The type of the button. Available options are 'button', 'submit', 'reset'.
     */
    htmlType?: ButtonHtmlType;
}

export function Button({
    icon,
    text,
    disabled,
    onClick,
    fullWidth,
    htmlType = ButtonHtmlType.Button,
    size = ButtonSize.Medium,
    type = ButtonType.Primary,
}: ButtonProps): React.JSX.Element {
    const paddingClasses = icon && !text ? PADDINGS_ONLY_ICON[size] : PADDINGS[size];
    const textSizes = TEXT_CLASSES[size];
    const backgroundColors = disabled ? DISABLED_BACKGROUND_COLORS[type] : BACKGROUND_COLORS[type];
    const textColors = disabled ? TEXT_COLOR_DISABLED[type] : TEXT_COLORS[type];
    return (
        <button
            onClick={onClick}
            type={htmlType}
            className={cx(
                'state-layer relative flex flex-row items-center justify-center gap-2 rounded-full disabled:opacity-40',
                paddingClasses,
                backgroundColors,
                fullWidth && 'w-full',
            )}
            disabled={disabled}
            type={htmlType}
        >
            {icon && <span className={cx(textColors)}>{icon}</span>}
            {text && <span className={cx('font-inter', textColors, textSizes)}>{text}</span>}
        </button>
    );
}
