// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TriangleDown } from '@iota/ui-icons';
import cx from 'classnames';
import { useCallback, useEffect } from 'react';
import { Dropdown } from '../dropdown/Dropdown';

interface SelectorFieldProps extends React.PropsWithChildren {
    /**
     * The field is disabled or not.
     */
    isDisabled?: boolean;
    /**
     * The field label.
     */
    label?: string;
    /**
     * The field caption.
     */
    caption?: string;
    /**
     * The dropdown elements to render.
     */
    dropdownElements?: React.ReactNode;
    /**
     * The icon to show on the left of the field.
     */
    leadingIcon?: React.ReactNode;
    /**
     * The supporting text to shown at the end of the selector.
     */
    supportingText?: string;
    /**
     * The error message to show under the field.
     */
    errorMessage?: string;
    /**
     * Is the dropdown open
     */
    isOpen?: boolean;
    /**
     * Set the dropdown open
     */
    setIsOpen?: (isOpen: boolean) => void;
}

export function SelectorField({
    isDisabled,
    label,
    leadingIcon,
    supportingText,
    errorMessage,
    caption,
    dropdownElements,
    children,
    isOpen,
    setIsOpen,
}: SelectorFieldProps) {
    const onClick = useCallback(() => {
        if (!isDisabled) {
            setIsOpen?.(!isOpen);
        }
    }, [isOpen, isDisabled]);

    useEffect(() => {
        if (isDisabled && isOpen) {
            setIsOpen?.(false);
        }
    }, [isDisabled]);

    return (
        <div
            aria-disabled={isDisabled}
            className={cx('group flex flex-col gap-y-2', {
                'opacity-40': isDisabled,
                errored: !!errorMessage,
                opened: isOpen,
            })}
        >
            {label && (
                <label
                    onClick={onClick}
                    className="text-label-lg text-neutral-40 dark:text-neutral-60"
                >
                    {label}
                </label>
            )}
            <div className="relative flex w-full flex-col">
                <button
                    onClick={onClick}
                    disabled={isDisabled}
                    className="flex flex-row items-center gap-x-3 rounded-lg border border-neutral-80 px-md py-sm hover:enabled:border-neutral-50 focus-visible:enabled:border-primary-30  active:enabled:border-primary-30 group-[.errored]:border-error-30 group-[.opened]:border-primary-30 dark:border-neutral-20 dark:hover:border-neutral-60 dark:group-[.errored]:border-error-80 dark:group-[.opened]:border-primary-80 [&:is(:focus,_:focus-visible,_:active)]:enabled:border-primary-30 dark:[&:is(:focus,_:focus-visible,_:active)]:enabled:border-primary-80"
                >
                    {leadingIcon && (
                        <span className="text-neutral-10 dark:text-neutral-92">{leadingIcon}</span>
                    )}
                    <div className="flex w-full flex-row items-baseline gap-x-3">
                        {children && (
                            <div className="block w-full text-start text-body-lg text-neutral-10 dark:text-neutral-92">
                                {children}
                            </div>
                        )}
                        {supportingText && (
                            <SecondaryText noErrorStyles>{supportingText}</SecondaryText>
                        )}
                    </div>
                    <TriangleDown
                        className={cx('text-neutral-10 transition-transform dark:text-neutral-92', {
                            ' rotate-180': isOpen,
                        })}
                        width={20}
                        height={20}
                    />
                </button>
                {isOpen && (
                    <div className="absolute top-full z-[2] w-full">
                        <Dropdown>{dropdownElements}</Dropdown>
                    </div>
                )}
            </div>
            <div className="flex flex-row items-center justify-between">
                {(errorMessage || caption) && (
                    <SecondaryText>{errorMessage || caption}</SecondaryText>
                )}
            </div>
        </div>
    );
}

function SecondaryText({
    children,
    noErrorStyles,
}: React.PropsWithChildren<{ noErrorStyles?: boolean }>) {
    const ERROR_STYLES = 'group-[.errored]:text-error-30 dark:group-[.errored]:text-error-80';
    return (
        <p
            className={cx('text-label-lg text-neutral-40  dark:text-neutral-60 ', {
                [ERROR_STYLES]: !noErrorStyles,
            })}
        >
            {children}
        </p>
    );
}
