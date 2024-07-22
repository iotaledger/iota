// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useRef, useState } from 'react';
import { TextFieldPropsByType, TextFieldTypeTextAreaProps } from './text-field.types';
import { TextFieldType } from './text-field.enums';
import { INPUT_CLASSES } from './text-field.classes';
import { TextFieldTrailingElement } from './TextFieldTrailingElement';
import cx from 'classnames';

export interface TextFieldBaseProps {
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
    showHideContentButton?: boolean;
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

export type TextFieldProps = TextFieldBaseProps & TextFieldPropsByType;

export function TextField({
    name,
    label,
    placeholder,
    caption,
    isDisabled,
    errorMessage,
    onChange,
    value,
    leadingIcon,
    supportingText,
    amountCounter,
    id,
    pattern,
    showHideContentButton,
    autofocus,
    trailingElement,
    required,
    ...inputProps
}: TextFieldProps) {
    const inputRef = useRef<HTMLInputElement>(null);
    const [isContentVisible, setIsContentVisible] = useState<boolean>(
        inputProps.type !== TextFieldType.Password,
    );

    const focusInput = useCallback(() => {
        inputRef.current?.focus();
    }, [inputRef]);

    const togglePasswordVisibility = useCallback(() => {
        setIsContentVisible(!isContentVisible);
        if (inputRef.current) {
            inputRef.current.type = isContentVisible ? TextFieldType.Password : TextFieldType.Text;
        }
    }, [inputProps, inputRef, isContentVisible]);

    function handleOnChange(e: React.ChangeEvent<HTMLTextAreaElement | HTMLInputElement>) {
        onChange?.(e.target.value, e.target.name);
    }

    return (
        <div
            aria-disabled={isDisabled}
            className={cx('group flex flex-col gap-y-2', {
                'opacity-40': isDisabled,
                errored: errorMessage,
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
                className={cx(
                    'flex flex-row items-center gap-x-3 rounded-lg border border-neutral-80 px-md py-sm  group-[.errored]:border-error-30 hover:group-[.enabled]:border-neutral-50  dark:border-neutral-60 dark:hover:border-neutral-60 dark:group-[.errored]:border-error-80 [&:has(input:focus)]:border-primary-30',
                    inputProps.type === TextFieldType.TextArea && !isContentVisible
                        ? 'cursor-auto select-none'
                        : 'group-[.enabled]:cursor-text',
                )}
                onClick={focusInput}
            >
                {leadingIcon && (
                    <span className="text-neutral-10 dark:text-neutral-92">{leadingIcon}</span>
                )}

                {inputProps.type !== TextFieldType.TextArea && (
                    <input
                        type={inputProps.type}
                        name={name}
                        placeholder={placeholder}
                        disabled={isDisabled}
                        value={value}
                        onChange={handleOnChange}
                        ref={inputRef}
                        required={required}
                        id={id}
                        pattern={pattern}
                        autoFocus={autofocus}
                        className={INPUT_CLASSES}
                    />
                )}

                {inputProps.type === TextFieldType.TextArea && (
                    <TextArea
                        name={name}
                        placeholder={placeholder}
                        isDisabled={isDisabled}
                        onChange={handleOnChange}
                        isContentVisible={isContentVisible}
                        value={value}
                        required={required}
                        id={id}
                        autofocus={autofocus}
                        {...inputProps}
                    />
                )}

                {supportingText && <SecondaryText noErrorStyles>{supportingText}</SecondaryText>}

                <TextFieldTrailingElement
                    value={value}
                    type={inputProps.type}
                    showHideContentButton={
                        inputProps.type === TextFieldType.Password
                            ? showHideContentButton ?? true
                            : showHideContentButton
                    }
                    onClearInput={() => onChange?.('')}
                    inputType={inputRef.current?.type}
                    toggleContentVisibility={togglePasswordVisibility}
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

type TextAreaProps = Pick<
    TextFieldProps,
    'isDisabled' | 'autofocus' | 'value' | 'name' | 'placeholder' | 'required' | 'id'
> &
    TextFieldTypeTextAreaProps;

function TextArea({
    isContentVisible,
    isDisabled,
    autofocus,
    value,
    hiddenRows,
    onChange,
    rows = 3,
    cols,
    name,
    placeholder,
    required,
    id,
}: {
    isContentVisible: boolean;
    onChange?: (e: React.ChangeEvent<HTMLTextAreaElement>) => void;
} & TextAreaProps) {
    return (
        <div className="relative w-full">
            <textarea
                disabled={isDisabled}
                placeholder={placeholder}
                required={required}
                id={id}
                name={name}
                autoFocus={autofocus}
                onChange={onChange}
                rows={rows}
                cols={cols}
                className={cx(INPUT_CLASSES, !isContentVisible && 'text-opacity-0')}
                value={value}
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
