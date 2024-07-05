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
import { Theme } from '@/lib/enums';
import { resolveThemedClasses } from '@/lib/utils';

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
     * The onClick event of the button.
     */
    onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;

    /**
     * Wheter to use the darkmode theme in this component.
     */
    darkmode?: boolean;
}

export function Button({
    icon,
    text,
    disabled,
    onClick,
    darkmode,
    size = ButtonSize.Medium,
    type = ButtonType.Primary,
}: ButtonProps): React.JSX.Element {
    const theme = darkmode ? Theme.Dark : Theme.Light;
    const paddingClasses = icon && !text ? PADDINGS_ONLY_ICON[size] : PADDINGS[size];
    const textSizes = TEXT_CLASSES[size];

    const backgroundColors = resolveThemedClasses(
        disabled ? DISABLED_BACKGROUND_COLORS[type] : BACKGROUND_COLORS[type],
        theme,
    );
    const textColors = resolveThemedClasses(
        disabled ? TEXT_COLOR_DISABLED[type] : TEXT_COLORS[type],
        theme,
    );
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
