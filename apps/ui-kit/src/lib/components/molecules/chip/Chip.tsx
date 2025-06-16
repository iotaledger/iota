// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { ChipState } from './chip.enums';
import {
    BORDER_CLASSES,
    BACKGROUND_CLASSES,
    ROUNDED_CLASS,
    STATE_LAYER_CLASSES,
    TEXT_COLOR,
    FOCUS_CLASSES,
} from './chip.classes';
import { ButtonUnstyled } from '@/components/atoms/button';
import { Close } from '@iota/apps-ui-icons';

interface ChipProps {
    /**
     * The label of the chip
     */
    label: string;
    /**
     * Whether to show the close icon
     */
    showClose?: boolean;
    /**
     * Whether the chip is selected
     */
    selected?: boolean;
    /**
     * Callback when the close icon is clicked
     */
    onClose?: () => void;
    /**
     * On Click handler for the chip
     */
    onClick?: () => void;
    /**
     * Avatar to show in the chip.
     */
    avatar?: React.JSX.Element;
    /**
     * Leading element to show in the chip.
     */
    leadingElement?: React.JSX.Element;
    /**
     * Trailing element to show in the chip.
     */
    trailingElement?: React.JSX.Element;
    /**
     * The button is disabled or not.
     */
    disabled?: boolean;
}

export function Chip({
    label,
    showClose,
    selected,
    onClose,
    onClick,
    avatar,
    leadingElement,
    trailingElement,
    disabled,
}: ChipProps) {
    const chipState = selected ? ChipState.Selected : ChipState.Default;
    return (
        <ButtonUnstyled
            onClick={onClick}
            className={cx(
                'border transition-all duration-500 ease-in-out disabled:opacity-40',
                ROUNDED_CLASS,
                BACKGROUND_CLASSES[chipState],
                BORDER_CLASSES[chipState],
                FOCUS_CLASSES,
            )}
            disabled={disabled}
        >
            <span
                className={cx(
                    'flex h-full w-full flex-row items-center gap-x-2',
                    avatar ? 'py-xxs' : 'py-[6px]',
                    avatar ? 'pl-xxs' : leadingElement ? 'pl-xs' : 'pl-sm',
                    ROUNDED_CLASS,
                    !disabled && STATE_LAYER_CLASSES,
                    showClose ? 'pr-xs' : 'pr-sm',
                    TEXT_COLOR[chipState],
                )}
            >
                {avatar ?? leadingElement}
                <span className="text-body-md">{label}</span>
                {trailingElement}
                {showClose && (
                    <ButtonUnstyled
                        onClick={onClose}
                        className="cursor-pointer [&_svg]:h-4 [&_svg]:w-4"
                    >
                        <Close />
                    </ButtonUnstyled>
                )}
            </span>
        </ButtonUnstyled>
    );
}
