// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useEffect } from 'react';
import cx from 'classnames';
import { Close } from '@iota/ui-icons';
import { SnackbarType } from './snackbar.enums';
import { BACKGROUND_COLOR, TEXT_COLOR } from '@/components/atoms/snackbar/snackbar.classes';

export interface SnackbarProps {
    /**
     * The message to display in the snackbar.
     */
    text: string;

    /**
     * State for the snackbar.
     */
    isOpen: boolean;

    /**
     * Type of the snackbar.
     */
    type: SnackbarType;

    /**
     * Duration in milliseconds for the snackbar to auto close. `0` will make the component not close automatically.
     */
    duration?: number;

    /**
     * Callback to close the snackbar.
     */
    onClose: () => void;

    /**
     * Show the close button.
     */
    showClose?: boolean;
}

export function Snackbar({
    text,
    isOpen = true,
    duration = 2000,
    onClose,
    showClose,
    type = SnackbarType.Default,
}: SnackbarProps) {
    useEffect(() => {
        if (isOpen && duration) {
            const timer = setTimeout(onClose, duration);
            return () => clearTimeout(timer);
        }
    }, [isOpen, duration, onClose]);

    const openedClass = isOpen ? 'translate-y-0 opacity-100' : 'translate-y-full opacity-0';

    return (
        <div
            className={cx(
                'transition-all duration-300 ease-out',
                'z-99 bottom-0',
                'flex w-full items-center justify-between gap-1 rounded-md py-sm pl-md pr-sm',
                BACKGROUND_COLOR[type],
                openedClass,
            )}
        >
            <p className={cx('text-left text-body-md', TEXT_COLOR[type])}>{text}</p>
            {showClose && (
                <Close
                    className={cx('h-5 w-5 cursor-pointer', TEXT_COLOR[type])}
                    onClick={onClose}
                />
            )}
        </div>
    );
}
