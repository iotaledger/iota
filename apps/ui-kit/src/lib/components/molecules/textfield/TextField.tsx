// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close, VisibilityOn, VisibilityOff } from '@iota/ui-icons';
import cx from 'classnames';
import { useCallback, useRef } from 'react';

enum TextFieldType {
    Text = 'text',
    Password = 'password',
    Email = 'email',
    Number = 'number',
}

interface TextFieldProps {
    /**
     * Name for the input field
     */
    name?: string;
    /**
     * Shows a label with the text above the input field.
     */
    label?: string;
    /**
     * The placeholder text inside the input field.
     */
    placeholder?: string;
    /**
     * Shows a caption with the text below the input field.
     */
    caption?: string;
    /**
     * Disables the input field
     */
    isDisabled?: boolean;
    /**
     * Shows an error message below the input field
     */
    errorMessage?: string;
    /**
     * Callback function that is called when the input field value changes
     */
    onChange?: (value: string, name?: string) => void;
    /**
     * The value of the input field
     */
    value?: string;
    /**
     * The type of the input field
     */
    type?: TextFieldType;
    /**
     * A leading element that is shown before the input field
     */
    leadingElement?: React.JSX.Element;
    /**
     * Supporting text that is shown at the side of the placeholder text.
     */
    supportingText?: string;
    /**
     * Amount counter that is shown at the side of the caption text.
     */
    amountCounter?: string | number;
    /**
     * Shows password toggle button
     */
    showPasswordToggle?: boolean;
    /**
     * The Id of the input field
     */
    id?: string;
    /**
     * Min number in case of number type
     */
    min?: number;
    /**
     * Max number in case of number type
     */
    max?: number;
    /**
     * Step number in case of number type
     */
    step?: number;
    /**
     * The pattern for the input field
     */
    pattern?: string;
}

export function TextField({
    name,
    label,
    placeholder,
    caption,
    isDisabled,
    errorMessage,
    onChange,
    value,
    type,
    leadingElement,
    supportingText,
    amountCounter,
    id,
    min,
    max,
    step,
    pattern,
    showPasswordToggle,
}: TextFieldProps) {
    const inputRef = useRef<HTMLInputElement>(null);

    const focusInput = useCallback(() => {
        inputRef.current?.focus();
    }, [inputRef]);

    const togglePasswordVisibility = useCallback(() => {
        if (inputRef.current) {
            inputRef.current.type =
                inputRef.current.type === TextFieldType.Password
                    ? TextFieldType.Text
                    : TextFieldType.Password;
        }
    }, [inputRef]);

    return (
        <div
            aria-disabled={isDisabled}
            className={cx('group flex flex-col gap-y-2', {
                'opacity-40': isDisabled,
                errored: errorMessage,
            })}
        >
            {label && (
                <label
                    onClick={focusInput}
                    htmlFor={id}
                    className="text-label-lg text-neutral-40 dark:text-neutral-60"
                >
                    {label}
                </label>
            )}
            <div
                className="flex cursor-text flex-row items-center gap-x-3 rounded-lg border border-neutral-80 px-md py-sm hover:border-neutral-50 group-[.errored]:border-error-30 group-[.invalid]:border-error-30 dark:border-neutral-60 dark:hover:border-neutral-60 dark:group-[.errored]:border-error-80 dark:group-[.invalid]:border-error-80 [&:has(input:focus-visible)]:border-primary-30"
                onClick={focusInput}
            >
                {leadingElement}
                <input
                    type={type}
                    name={name}
                    placeholder={placeholder}
                    disabled={isDisabled}
                    value={value}
                    onChange={(e) => onChange?.(e.target.value)}
                    ref={inputRef}
                    id={id}
                    min={min}
                    max={max}
                    step={step}
                    pattern={pattern}
                    className="w-full text-body-lg text-neutral-10 caret-primary-30 focus:outline-none focus-visible:outline-none enabled:placeholder:text-neutral-40/40 dark:text-neutral-92 enabled:dark:placeholder:text-neutral-60/40"
                />
                {supportingText && <SecondaryText>{supportingText}</SecondaryText>}
                {type === TextFieldType.Password && showPasswordToggle && (
                    <button onClick={togglePasswordVisibility}>
                        {inputRef.current?.type === TextFieldType.Password ? (
                            <VisibilityOn onClick={togglePasswordVisibility} />
                        ) : (
                            <VisibilityOff onClick={togglePasswordVisibility} />
                        )}
                    </button>
                )}
                {type !== TextFieldType.Password && value && (
                    <button onClick={() => onChange?.('')}>
                        <Close />
                    </button>
                )}
            </div>
            <div className="flex flex-row items-center justify-between">
                {errorMessage ? (
                    <span className="text-label-lg">{errorMessage}</span>
                ) : (
                    caption && <SecondaryText>{caption}</SecondaryText>
                )}
                {amountCounter && <SecondaryText>{amountCounter}</SecondaryText>}
            </div>
        </div>
    );
}

function SecondaryText({ children }: React.PropsWithChildren) {
    return (
        <p className="text-label-lg text-neutral-40 group-[.errored]:text-error-30 dark:text-neutral-60 dark:group-[.errored]:text-error-80">
            {children}
        </p>
    );
}
