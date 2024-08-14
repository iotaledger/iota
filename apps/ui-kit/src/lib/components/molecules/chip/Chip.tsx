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
import { ButtonUnstyled } from '../../atoms/button/ButtonUnstyled';
import { Close } from '@iota/ui-icons';

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
     * Avatar to show in the chip.
     */
    avatar?: React.JSX.Element;
    /**
     * Icon to show in the chip.
     */
    icon?: React.JSX.Element;
}

export function Chip({ label, showClose, selected, onClose, avatar, icon }: ChipProps) {
    const chipState = selected ? ChipState.Selected : ChipState.Default;
    return (
        <ButtonUnstyled
            className={cx(
                'border',
                ROUNDED_CLASS,
                BACKGROUND_CLASSES[chipState],
                BORDER_CLASSES[chipState],
                FOCUS_CLASSES,
            )}
        >
            <span
                className={cx(
                    'flex h-full w-full flex-row items-center gap-x-2',
                    avatar ? 'py-xxs' : 'py-[6px]',
                    avatar ? 'pl-xxs' : icon ? 'pl-xs' : 'pl-sm',
                    ROUNDED_CLASS,
                    STATE_LAYER_CLASSES,
                    showClose ? 'pr-xs' : 'pr-sm',
                    TEXT_COLOR[chipState],
                )}
            >
                {avatar ?? icon}
                <span className="text-body-md">{label}</span>
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
