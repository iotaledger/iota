// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useEffect } from 'react';
import cx from 'classnames';
import { Close } from '@iota/ui-icons';
import { SnackbarType } from './snackbar.enums';

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

    return (
        <div
            className={cx(
                'transition-all duration-300 ease-out',
                'z-99 bottom-0',
                'flex w-full items-center justify-between gap-1 rounded-md px-md py-sm',
                {
                    'bg-neutral-80 dark:bg-neutral-30': type === SnackbarType.Default,
                    'bg-error-90 dark:bg-error-10': type === SnackbarType.Error,
                    'translate-y-0 opacity-100': isOpen,
                    'translate-y-full opacity-0': !isOpen,
                },
            )}
        >
            <p
                className={cx('text-left text-body-md dark:text-neutral-60', {
                    'text-neutral-10 dark:text-neutral-92': type === SnackbarType.Default,
                    'text-error-20 dark:text-error-90': type === SnackbarType.Error,
                })}
            >
                {text}
            </p>
            {showClose && (
                <Close className={'h-5 w-5 cursor-pointer text-neutral-6'} onClick={onClose} />
            )}
        </div>
    );
}
