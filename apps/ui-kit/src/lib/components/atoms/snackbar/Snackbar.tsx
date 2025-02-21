// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';
import cx from 'classnames';
import { Close } from '@iota/apps-ui-icons';
import { SnackbarType } from './snackbar.enums';
import { BACKGROUND_COLOR, TEXT_COLOR } from '@/components/atoms/snackbar/snackbar.classes';

type Renderable = JSX.Element | string | null;
export interface SnackbarProps {
    /**
     * The message to display in the snackbar.
     */
    text: Renderable;
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
    duration = 2000,
    onClose,
    showClose,
    type = SnackbarType.Default,
}: SnackbarProps) {
    useEffect(() => {
        if (duration) {
            const timer = setTimeout(onClose, duration);
            return () => clearTimeout(timer);
        }
    }, [duration, onClose]);

    return (
        <div
            className={cx(
                'transition-all duration-300 ease-out',
                'z-99 bottom-0',
                'flex w-full items-center justify-between gap-xxs rounded-md py-sm pl-md pr-sm',
                BACKGROUND_COLOR[type],
            )}
        >
            <div className={cx('w-full text-left text-body-md', TEXT_COLOR[type])}>{text}</div>
            {showClose && (
                <Close
                    className={cx('h-5 w-5 cursor-pointer', TEXT_COLOR[type])}
                    onClick={onClose}
                />
            )}
        </div>
    );
}
