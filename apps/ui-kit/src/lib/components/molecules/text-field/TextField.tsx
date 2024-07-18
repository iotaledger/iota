// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close, VisibilityOn, VisibilityOff } from '@iota/ui-icons';
import cx from 'classnames';
import { useCallback, useRef } from 'react';
import { TextFieldPropsByType } from './text-field.types';
import { TextFieldType } from './text-field.enums';

interface TextFieldBaseProps {
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
     * Adds error styling to the input field
     */
    isErrored?: boolean;

    /**
     * Error Message. Overrides the caption.
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
     * A leading icon that is shown before the input field
     */
    leadingIcon?: React.JSX.Element;
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
    hidePasswordToggle?: boolean;
    /**
     * The Id of the input field
     */
    id?: string;
    /**
     * The pattern for the input field
     */
    pattern?: string;
    /**
     * Autofocus the input field on render
     */
    autofocus?: boolean;
    /**
     * Trailing element that is shown after the input field
     */
    trailingElement?: React.JSX.Element;
    /**
     * If the field is required
     */
    required?: boolean;
}

type TextFieldProps = TextFieldBaseProps & TextFieldPropsByType;

export function TextField({
    name,
    label,
    placeholder,
    caption,
    isDisabled,
    isErrored,
    errorMessage,
    onChange,
    value,
    leadingIcon,
    supportingText,
    amountCounter,
    id,
    pattern,
    hidePasswordToggle,
    autofocus,
    trailingElement,
    required,
    type = TextFieldType.Text,
    ...inputPropsByType
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
                errored: isErrored,
                enabled: !isDisabled,
                required: required,
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
                className="flex flex-row items-center gap-x-3 rounded-lg border border-neutral-80 px-md py-sm group-[.enabled]:cursor-text group-[.errored]:border-error-30 hover:group-[.enabled]:border-neutral-50  dark:border-neutral-60 dark:hover:border-neutral-60 dark:group-[.errored]:border-error-80 [&:has(input:focus)]:border-primary-30"
                onClick={focusInput}
            >
                {leadingIcon && (
                    <span className="text-neutral-10 dark:text-neutral-92">{leadingIcon}</span>
                )}
                <input
                    type={type}
                    name={name}
                    placeholder={placeholder}
                    disabled={isDisabled}
                    value={value}
                    onChange={(e) => onChange?.(e.target.value)}
                    ref={inputRef}
                    required={required}
                    id={id}
                    {...inputPropsByType}
                    pattern={pattern}
                    autoFocus={autofocus}
                    className="w-full bg-transparent text-body-lg text-neutral-10 caret-primary-30 focus:outline-none focus-visible:outline-none enabled:placeholder:text-neutral-40/40 dark:text-neutral-92 dark:placeholder:text-neutral-60/40 enabled:dark:placeholder:text-neutral-60/40"
                />
                {supportingText && <SecondaryText noErrorStyles>{supportingText}</SecondaryText>}

                <TextFieldTrailingElement
                    value={value}
                    type={type}
                    hidePasswordToggle={hidePasswordToggle}
                    onChange={onChange}
                    inputRef={inputRef}
                    togglePasswordVisibility={togglePasswordVisibility}
                    trailingElement={trailingElement}
                />
            </div>
            <div className="flex flex-row items-center justify-between">
                {caption && <SecondaryText>{errorMessage ?? caption}</SecondaryText>}
                {amountCounter && <SecondaryText>{amountCounter}</SecondaryText>}
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

type TextFieldTrailingElement = Pick<
    TextFieldProps,
    'value' | 'type' | 'hidePasswordToggle' | 'onChange' | 'trailingElement'
> & {
    inputRef: React.RefObject<HTMLInputElement>;
    togglePasswordVisibility: () => void;
};

function TextFieldTrailingElement({
    value,
    type,
    hidePasswordToggle,
    onChange,
    inputRef,
    togglePasswordVisibility,
    trailingElement,
}: TextFieldTrailingElement) {
    if (trailingElement) {
        return trailingElement;
    }

    if (type === TextFieldType.Password && !hidePasswordToggle) {
        return (
            <button
                onClick={togglePasswordVisibility}
                className="text-neutral-10 dark:text-neutral-92"
            >
                {inputRef.current?.type === TextFieldType.Password ? (
                    <VisibilityOn />
                ) : (
                    <VisibilityOff />
                )}
            </button>
        );
    }

    if ((type === TextFieldType.Text || type === TextFieldType.Email) && value) {
        return (
            <button className="text-neutral-10 dark:text-neutral-92" onClick={() => onChange?.('')}>
                <Close />
            </button>
        );
    }
}
