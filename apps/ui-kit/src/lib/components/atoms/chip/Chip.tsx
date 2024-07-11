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
} from './chip.classes';

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
     * Leading element to be displayed before the label.
     */
    leadingElement?: React.ReactNode;
}

export function Chip({ label, showClose, selected, onClose, leadingElement }: ChipProps) {
    const chipState = selected ? ChipState.Selected : ChipState.Default;
    return (
        <div
            className={cx(
                'border',
                ROUNDED_CLASS,
                BACKGROUND_CLASSES[chipState],
                BORDER_CLASSES[chipState],
            )}
        >
            <span
                className={cx(
                    'flex h-full w-full flex-row items-center gap-x-2 py-[6px]',
                    ROUNDED_CLASS,
                    STATE_LAYER_CLASSES,
                    leadingElement ? 'pl-xs' : 'pl-sm',
                    showClose ? 'pr-xs' : 'pr-sm',
                    TEXT_COLOR[chipState],
                )}
            >
                {leadingElement}
                <span className="text-body-md">{label}</span>
                {showClose && (
                    <span onClick={onClose} className="cursor-pointer text-body-md">
                        &#x2715;
                    </span>
                )}
            </span>
        </div>
    );
}
