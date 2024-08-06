// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';

export interface InputWrapperProps {
    /**
     * Shows a label with the text above the input.
     */
    label?: string;
    /**
     * Shows a caption with the text below the input.
     */
    caption?: string;
    /**
     * Error Message. Overrides the caption.
     */
    errorMessage?: string;
    /**
     * Amount counter that is shown at the side of the caption text.
     */
    amountCounter?: string | number;
    /**
     * Is the input required
     */
    required?: boolean;
    /**
     * Is the input disabled
     */
    disabled?: boolean;
}

export function InputWrapper({
    label,
    caption,
    disabled,
    errorMessage,
    amountCounter,
    required,
    children,
}: React.PropsWithChildren<InputWrapperProps>) {
    return (
        <div
            className={cx('group flex flex-col gap-y-2', {
                'opacity-40': disabled,
                errored: errorMessage,
                enabled: !disabled,
                required: required,
            })}
        >
            {label ? (
                <label className="flex flex-col gap-y-2 text-label-lg text-neutral-40 dark:text-neutral-60">
                    {label}
                    {children}
                </label>
            ) : (
                children
            )}

            <div
                className={cx(
                    'flex flex-row items-center',
                    caption || errorMessage ? 'justify-between' : 'justify-end',
                )}
            >
                {(errorMessage || caption) && (
                    <SecondaryText>{errorMessage || caption}</SecondaryText>
                )}
                {amountCounter && <SecondaryText>{amountCounter}</SecondaryText>}
            </div>
        </div>
    );
}

export function SecondaryText({
    children,
    noErrorStyles,
    className,
}: React.PropsWithChildren<{ noErrorStyles?: boolean; className?: string }>) {
    const ERROR_STYLES = 'group-[.errored]:text-error-30 dark:group-[.errored]:text-error-80';
    return (
        <p
            className={cx(
                'text-label-lg text-neutral-40  dark:text-neutral-60 ',
                {
                    [ERROR_STYLES]: !noErrorStyles,
                },
                className,
            )}
        >
            {children}
        </p>
    );
}
