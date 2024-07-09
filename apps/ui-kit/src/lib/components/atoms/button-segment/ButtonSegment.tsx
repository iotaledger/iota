// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { BACKGROUND_COLORS, TEXT_COLORS, TEXT_SELECTED } from './button-segment.classes';
import cx from 'classnames';

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
}

export function ButtonSegment({
    icon,
    label,
    selected = false,
    disabled,
    onClick,
}: ButtonSegmentProps): React.JSX.Element {
    const textColors = selected ? TEXT_SELECTED : TEXT_COLORS;
    return (
        <button
            onClick={onClick}
            className={cx(
                'relative flex rounded-full px-md py-sm disabled:opacity-40',
                textColors,
                BACKGROUND_COLORS,
            )}
            disabled={disabled}
        >
            <div className={cx('flex flex-row items-center justify-center gap-2')}>
                {icon && <span>{icon}</span>}
                <span className={cx('font-inter text-label-md')}>{label}</span>
            </div>
        </button>
    );
}
