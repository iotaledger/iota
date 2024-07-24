// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useEffect } from 'react';
import cx from 'classnames';
import { Close } from '@iota/ui-icons';
import { ButtonType, SnackbarType } from '@/lib';
import { Button } from '../button';

export interface SnackbarProps {
    /**
     * The message to display in the snackbar.
     */
    message: string;

    /**
     * State for the snackbar.
     */
    isOpen: boolean;

    /**
     * Change UI if the message is multiline.
     */
    isMultiline?: boolean;

    /**
     * Type of the snackbar.
     */
    type: SnackbarType;

    /**
     * Duration in milliseconds for the snackbar to auto close.
     */
    duration?: number;

    /**
     * Callback to close the snackbar.
     */
    onClose: () => void;

    /**
     * Optional action to display in the snackbar.
     * If action is provided - label & onClick are required.
     */
    action?: {
        label: string;
        onClick: () => void;
    };
}

function Snackbar({
    message,
    isOpen = true,
    duration = 0, // 0 means it will not auto close
    onClose,
    isMultiline,
    type = SnackbarType.Default,
    action,
}: SnackbarProps) {
    useEffect(() => {
        if (isOpen && duration) {
            const timer = setTimeout(onClose, duration);
            return () => clearTimeout(timer);
        }
    }, [isOpen, duration, onClose]);

    // Using classnames to manage dynamic classes
    const snackbarClasses = cx(
        'bottom-0 flex w-full z-99 gap-1 justify-between items-center',
        'rounded-md text-left',
        'transition-opacity transition-transform duration-300 ease-out',
        {
            'bg-neutral-80': type === SnackbarType.Default,
            'bg-tertiary-90': type === SnackbarType.Success,
            'bg-error-90': type === SnackbarType.Error,
            'translate-y-0 opacity-100': isOpen,
            'translate-y-full opacity-0': !isOpen,
            'py-[14px] px-md': !action && !onClose,
            'py-[6px] pl-md pr-xs': action || onClose,
            'pr-none': !isMultiline,
            'pr-none pb-none': isMultiline,
            'flex-col': isMultiline,
        },
    );

    return (
        <div className={snackbarClasses}>
            <div className={cx({ 'w-full': isMultiline })}>
                <p className="text-left text-body-md text-neutral-10 dark:text-neutral-60">
                    {message}
                </p>
            </div>
            <div
                className={cx('flex gap-1 whitespace-nowrap', {
                    'w-full justify-end': isMultiline,
                })}
            >
                {action && (
                    <Button type={ButtonType.Ghost} onClick={action.onClick} text={action.label} />
                )}
                {onClose && (
                    <Button
                        type={ButtonType.Ghost}
                        onClick={onClose}
                        icon={<Close width={16} height={16} />}
                    />
                )}
            </div>
        </div>
    );
}

export { Snackbar };
