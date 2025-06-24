// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { ChipType } from './chip.enums';
import {
    BORDER_CLASSES,
    BACKGROUND_CLASSES,
    ROUNDED_CLASS,
    STATE_LAYER_CLASSES,
    TEXT_COLOR,
    FOCUS_CLASSES,
    TEXT_COLOR_SELECTED,
    BACKGROUND_CLASSES_SELECTED,
    SELECTED_OVERLAY,
} from './chip.classes';
import { ButtonUnstyled } from '@/components/atoms/button';
import { Close } from '@iota/apps-ui-icons';

interface ChipProps {
    label: string;
    type?: ChipType;
    selected?: boolean;
    disabled?: boolean;
    showClose?: boolean;
    onClose?: () => void;
    onClick?: () => void;
    avatar?: React.JSX.Element;
    leadingElement?: React.JSX.Element;
    trailingElement?: React.JSX.Element;
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
        (type === ChipType.Outline && selected) || (type !== ChipType.Outline && !selected)
            ? 'outline-transparent'
            : 'chip-outline-color';

    return (
        <ButtonUnstyled
            onClick={onClick}
            className={cx(
                'border transition-all duration-500 ease-in-out disabled:opacity-40',
                ROUNDED_CLASS,
                isOutlineSelected
                    ? BACKGROUND_CLASSES_SELECTED[ChipType.Outline]
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
                    isOutlineSelected ? TEXT_COLOR_SELECTED[ChipType.Outline] : TEXT_COLOR[type],
                    outlineStyle,
                    !disabled && STATE_LAYER_CLASSES,
                    selected && !disabled && type !== ChipType.Outline && SELECTED_OVERLAY,
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
