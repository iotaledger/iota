// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { BACKGROUND_COLORS, TEXT_COLORS, TEXT_SELECTED } from './segment-button.classes';
import cx from 'classnames';

interface SegmentButtonProps {
    /**
     * The text of the button.
     */
    text: string;
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
}

export function SegmentButton({
    icon,
    text,
    selected = false,
    disabled,
    onClick,
}: SegmentButtonProps): React.JSX.Element {
    const textColors = selected ? TEXT_SELECTED : TEXT_COLORS;
    return (
        <button
            onClick={onClick}
            className={cx('relative flex rounded-full px-md py-sm', textColors, BACKGROUND_COLORS)}
            disabled={disabled}
        >
            <div className={cx('flex flex-row items-center justify-center gap-2')}>
                {icon && <span className={cx()}>{icon}</span>}
                <span className={cx('font-inter text-label-md')}>{text}</span>
            </div>
        </button>
    );
}
