// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonUnstyled } from '@/components/atoms/button';
import { Close } from '@iota/apps-ui-icons';
import cx from 'classnames';
import {
    BACKGROUND_CLASSES,
    BG_SELECTED_OUTLINE,
    BG_SELECTED_OVERLAY,
    BORDER_CLASSES,
    FOCUS_CLASSES,
    ROUNDED_CLASS,
    STATE_LAYER_CLASSES,
    TEXT_COLOR,
    TEXT_COLOR_SELECTED_OUTLINE,
} from './chip.classes';
import { ChipType } from './chip.enums';

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
    /**
     * The type of the Chip
     */
    type?: ChipType;
}

export function Chip({
    label,
    type = ChipType.Outline,
    selected,
    disabled,
    showClose,
    onClose,
    onClick,
    avatar,
    leadingElement,
    trailingElement,
}: ChipProps) {
    const isOutlineSelected = type === ChipType.Outline && selected;
    const outlineStyle =
        !selected || (type === ChipType.Outline && selected)
            ? 'outline-transparent'
            : 'chip-outline-color';
    const selectedOverlayBg =
        selected && !disabled && type !== ChipType.Outline ? BG_SELECTED_OVERLAY : '';

    return (
        <ButtonUnstyled
            onClick={onClick}
            className={cx(
                'group border transition-all duration-500 ease-in-out disabled:opacity-40',
                ROUNDED_CLASS,
                isOutlineSelected
                    ? BG_SELECTED_OUTLINE[ChipType.Outline]
                    : BACKGROUND_CLASSES[type],
                selected ? 'border-transparent' : BORDER_CLASSES[type],
                FOCUS_CLASSES,
            )}
            disabled={disabled}
        >
            <span
                className={cx(
                    'flex h-full w-full flex-row items-center gap-x-2',
                    avatar ? 'py-xxs' : 'py-[6px]',
                    avatar ? 'pl-xxs' : leadingElement ? 'pl-xs' : 'pl-sm',
                    showClose ? 'pr-xs' : 'pr-sm',
                    ROUNDED_CLASS,
                    isOutlineSelected
                        ? TEXT_COLOR_SELECTED_OUTLINE[ChipType.Outline]
                        : TEXT_COLOR[type],
                    outlineStyle,
                    selectedOverlayBg,
                    !disabled && STATE_LAYER_CLASSES,
                )}
            >
                {avatar ?? leadingElement}
                <span className="text-body-md">{label}</span>
                {trailingElement}
                {showClose && (
                    <ButtonUnstyled onClick={onClose} className="cursor-pointer">
                        <Close
                            className={cx(
                                'h-4 w-4 transition-all duration-500 ease-in-out',
                                selected
                                    ? 'opacity-100'
                                    : 'opacity-40 group-hover:opacity-100 group-focus:opacity-100 group-active:opacity-100',
                            )}
                        />
                    </ButtonUnstyled>
                )}
            </span>
        </ButtonUnstyled>
    );
}
