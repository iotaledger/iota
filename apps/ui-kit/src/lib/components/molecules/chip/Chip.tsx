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
    CLOSE_ICON_INTERACTIVE,
    FOCUS_CLASSES,
    ROUNDED_CLASS,
    STATE_LAYER_CLASSES,
    TEXT_COLOR,
    TEXT_COLOR_SELECTED_OUTLINE,
} from './chip.classes';
import { ChipSize, ChipType } from './chip.enums';

interface ChipProps extends React.AriaAttributes {
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
    /**
     * The size of the chip.
     */
    size?: ChipSize;
}

export function Chip({
    label,
    type = ChipType.Outline,
    selected,
    showClose,
    onClose,
    onClick,
    avatar,
    leadingElement,
    trailingElement,
    disabled,
    size = ChipSize.Default,
    ...ariaProps
}: ChipProps) {
    const isOutlineSelected = type === ChipType.Outline && selected;
    const outlineStyle = selected
        ? type === ChipType.Outline
            ? 'outline-transparent names:outline-names-neutral-20'
            : 'chip-outline-color'
        : 'outline-transparent';

    const focusOutlineStyle = cx(
        'group-focus:chip-outline-color-active',
        type === ChipType.Outline && 'names:group-focus:outline-transparent',
    );
    const selectedOverlayBg =
        selected && !disabled && type !== ChipType.Outline ? BG_SELECTED_OVERLAY : '';

    return (
        <ButtonUnstyled
            onClick={onClick}
            className={cx(
                'group border disabled:opacity-40',
                ROUNDED_CLASS,
                isOutlineSelected
                    ? BG_SELECTED_OUTLINE[ChipType.Outline]
                    : BACKGROUND_CLASSES[type],
                selected ? 'border-transparent' : BORDER_CLASSES[type],
            )}
            disabled={disabled}
            {...ariaProps}
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
                    !disabled && focusOutlineStyle,
                    !disabled && STATE_LAYER_CLASSES,
                    FOCUS_CLASSES,
                    selectedOverlayBg,
                )}
            >
                {avatar ?? leadingElement}
                <span className={cx(size === ChipSize.Small ? 'text-label-md' : 'text-body-md')}>
                    {label}
                </span>
                {trailingElement}
                {showClose && (
                    <ButtonUnstyled
                        onClick={(e) => {
                            e.stopPropagation();
                            onClose?.();
                        }}
                        disabled={disabled}
                        aria-label="Remove"
                    >
                        <Close
                            className={cx(
                                'h-4 w-4 transition-opacity duration-150',
                                disabled ? '' : selected ? 'opacity-100' : CLOSE_ICON_INTERACTIVE,
                            )}
                        />
                    </ButtonUnstyled>
                )}
            </span>
        </ButtonUnstyled>
    );
}
