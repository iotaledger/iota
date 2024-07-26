// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRef } from 'react';
import { TextFieldPropsByType, TextFieldTypeTextAreaProps } from './text-field.types';
import { TextFieldType } from './text-field.enums';
import { INPUT_CLASSES } from './text-field.classes';
import { TextFieldTrailingElement } from './TextFieldTrailingElement';
import cx from 'classnames';

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
     * Shows toggle button to show/hide the content of the input field
     */
    isToggleButtonVisible?: boolean;
    /**
     * Trailing element that is shown after the input field
     */
    trailingElement?: React.JSX.Element;
    /**
     * Ref for the input field
     */
    ref?: React.RefObject<HTMLInputElement>;
    /**
     * Is the content of the input visible
     */
    isContentVisible?: boolean;
    /**
     * Value of the input field
     */
    value?: string;
    /**
     * Toggles the password visibility
     */
    onToggleButtonClick?: () => void;
    /**
     * onClearInput function that is called when the clear button is clicked
     */
    onClearInput?: () => void;
}

export type TextFieldProps = TextFieldBaseProps & TextFieldPropsByType;

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
    isToggleButtonVisible,
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
    isContentVisible,
    onToggleButtonClick,
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
                className={cx(
                    'flex flex-row items-center gap-x-3 rounded-lg border border-neutral-80 px-md py-sm  group-[.errored]:border-error-30 hover:group-[.enabled]:border-neutral-50  dark:border-neutral-60 dark:hover:border-neutral-60 dark:group-[.errored]:border-error-80 [&:has(input:focus)]:border-primary-30',
                    type === TextFieldType.TextArea && !isContentVisible
                        ? 'cursor-auto select-none'
                        : 'group-[.enabled]:cursor-text',
                )}
                onClick={focusInput}
            >
                {leadingIcon && (
                    <span className="text-neutral-10 dark:text-neutral-92">{leadingIcon}</span>
                )}

                {type !== TextFieldType.TextArea && (
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
                )}

                {type === TextFieldType.TextArea && (
                    <TextArea
                        name={name}
                        placeholder={placeholder}
                        disabled={disabled}
                        onChange={onChange}
                        isContentVisible={isContentVisible}
                        value={value}
                        required={required}
                        id={id}
                        autoFocus={autoFocus}
                        maxLength={maxLength}
                        minLength={minLength}
                    />
                )}

                {supportingText && <SecondaryText noErrorStyles>{supportingText}</SecondaryText>}

                <TextFieldTrailingElement
                    value={value}
                    type={type}
                    isToggleButtonVisible={
                        type === TextFieldType.Password
                            ? isToggleButtonVisible ?? true
                            : isToggleButtonVisible
                    }
                    onClearInput={() => onChange?.('')}
                    inputType={inputRef.current?.type}
                    onToggleButtonClick={onToggleButtonClick}
                    trailingElement={trailingElement}
                    isContentVisible={isContentVisible}
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

type TextAreaProps = Pick<
    TextFieldProps,
    | 'disabled'
    | 'autoFocus'
    | 'value'
    | 'name'
    | 'placeholder'
    | 'required'
    | 'id'
    | 'maxLength'
    | 'minLength'
> &
    Omit<TextFieldTypeTextAreaProps, 'type'>;

function TextArea({
    isContentVisible,
    disabled,
    autoFocus,
    value,
    hiddenRows,
    onChange,
    rows = 3,
    cols,
    name,
    placeholder,
    required,
    id,
    maxLength,
    minLength,
}: {
    isContentVisible?: boolean;
    onChange?: (value: string, name?: string) => void;
} & TextAreaProps) {
    return (
        <div className="relative w-full">
            <textarea
                disabled={disabled}
                placeholder={placeholder}
                required={required}
                id={id}
                name={name}
                autoFocus={autoFocus}
                onChange={(e) => onChange?.(e.target.value, e.target.name)}
                rows={rows}
                cols={cols}
                className={cx(INPUT_CLASSES, !isContentVisible && 'text-opacity-0')}
                value={value}
                maxLength={maxLength}
                minLength={minLength}
            />

            {!isContentVisible && (
                <div className="absolute left-0 top-0 flex h-full w-full flex-col items-stretch gap-y-2">
                    {new Array(hiddenRows ?? rows ?? 3).fill(0).map((_, index) => (
                        <div key={index} className="h-full w-full rounded bg-neutral-92" />
                    ))}
                </div>
            )}
        </div>
    );
}
