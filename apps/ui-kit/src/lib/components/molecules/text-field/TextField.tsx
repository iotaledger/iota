// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { Close, VisibilityOn, VisibilityOff } from '@iota/ui-icons';
import { useRef } from 'react';
import { TextFieldPropsByType } from './text-field.types';
import { TextFieldType } from './text-field.enums';

type InputPickedProps = Pick<
    React.InputHTMLAttributes<HTMLInputElement>,
    | 'min'
    | 'max'
    | 'step'
    | 'maxLength'
    | 'minLength'
    | 'autoComplete'
    | 'autoFocus'
    | 'pattern'
    | 'name'
    | 'required'
    | 'placeholder'
    | 'disabled'
    | 'id'
>;

interface TextFieldBaseProps extends InputPickedProps {
    /**
     * Shows a label with the text above the input field.
     */
    label?: string;
    /**
     * Shows a caption with the text below the input field.
     */
    caption?: string;
    /**
     * Error Message. Overrides the caption.
     */
    errorMessage?: string;
    /**
     * Callback function that is called when the input field value changes
     */
    onChange?: (value: string, name?: string) => void;
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
     * Trailing element that is shown after the input field
     */
    trailingElement?: React.JSX.Element;
    /**
     * Ref for the input field
     */
    ref?: React.RefObject<HTMLInputElement>;
    /**
     * Is the password visible
     */
    isPasswordVisible?: boolean;
    /**
     * Value of the input field
     */
    value?: string;
    /**
     * Toggles the password visibility
     */
    togglePasswordVisibility?: () => void;
    /**
     * onClearInput function that is called when the clear button is clicked
     */
    onClearInput?: () => void;
}

type TextFieldProps = TextFieldBaseProps & TextFieldPropsByType;

export function TextField({
    name,
    label,
    placeholder,
    caption,
    disabled,
    errorMessage,
    onChange,
    value,
    leadingIcon,
    supportingText,
    amountCounter,
    id,
    pattern,
    hidePasswordToggle,
    autoFocus,
    trailingElement,
    required,
    max,
    min,
    step,
    maxLength,
    minLength,
    autoComplete,
    ref,
    isPasswordVisible,
    togglePasswordVisibility,
    onClearInput,
    type = TextFieldType.Text,
}: TextFieldProps) {
    const fallbackRef = useRef<HTMLInputElement>(null);
    const inputRef = ref ?? fallbackRef;

    function focusInput() {
        if (inputRef?.current) {
            inputRef?.current?.focus();
        }
    }

    return (
        <div
            aria-disabled={disabled}
            className={cx('group flex flex-col gap-y-2', {
                'opacity-40': disabled,
                errored: errorMessage,
                enabled: !disabled,
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
                    disabled={disabled}
                    value={value}
                    onChange={(e) => onChange?.(e.target.value, e.target.name)}
                    ref={inputRef}
                    required={required}
                    id={id}
                    pattern={pattern}
                    autoFocus={autoFocus}
                    maxLength={maxLength}
                    minLength={minLength}
                    autoComplete={autoComplete}
                    max={max}
                    min={min}
                    step={step}
                    className="w-full bg-transparent text-body-lg text-neutral-10 caret-primary-30 focus:outline-none focus-visible:outline-none enabled:placeholder:text-neutral-40/40 dark:text-neutral-92 dark:placeholder:text-neutral-60/40 enabled:dark:placeholder:text-neutral-60/40"
                />
                {supportingText && <SecondaryText noErrorStyles>{supportingText}</SecondaryText>}

                <TextFieldTrailingElement
                    value={value}
                    type={type}
                    hidePasswordToggle={hidePasswordToggle}
                    onClearInput={onClearInput}
                    togglePasswordVisibility={togglePasswordVisibility}
                    trailingElement={trailingElement}
                    isPasswordVisible={isPasswordVisible}
                />
            </div>
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

function SecondaryText({
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

type TextFieldTrailingElement = Pick<
    TextFieldProps,
    'value' | 'type' | 'hidePasswordToggle' | 'trailingElement' | 'isPasswordVisible'
> & {
    togglePasswordVisibility?: () => void;
    onClearInput?: () => void;
};

function TextFieldTrailingElement({
    value,
    type,
    hidePasswordToggle,
    onClearInput,
    togglePasswordVisibility,
    trailingElement,
    isPasswordVisible,
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
                {isPasswordVisible ? <VisibilityOn /> : <VisibilityOff />}
            </button>
        );
    }

    if (type === TextFieldType.Text && value) {
        return (
            <button className="text-neutral-10 dark:text-neutral-92" onClick={onClearInput}>
                <Close />
            </button>
        );
    }
}
