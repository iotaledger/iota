// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from 'react';
import { InputFieldTrailingElement } from './InputFieldTrailingElement';
import cx from 'classnames';
import { InputFieldWrapper, InputFieldWrapperProps, SecondaryText } from './InputFieldWrapper';
import {
    BORDER_CLASSES,
    INPUT_CLASSES,
    INPUT_TEXT_CLASSES,
    INPUT_NUMBER_CLASSES,
    INPUT_PLACEHOLDER_CLASSES,
} from './input-field.classes';
import { InputFieldType } from './input-field.enums';
import { InputFieldPropsByType } from './input-field.types';

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

interface InputFieldBaseProps extends InputPickedProps, InputFieldWrapperProps {
    /**
     * Callback function that is called when the input field value changes
     */
    onChange?: (value: string, name?: string) => void;
    /**
     * A leading icon that is shown before the input field
     */
    leadingIcon?: React.JSX.Element;
    /**
     * Supporting text that is shown at the end of the input component.
     */
    supportingText?: string;
    /**
     * Amount counter that is shown at the side of the caption text.
     */
    amountCounter?: string | number;
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
     * onClearInput function that is called when the clear button is clicked
     */
    onClearInput?: () => void;
}

export type InputFieldProps = InputFieldBaseProps & InputFieldPropsByType;

export function InputField({
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
    onClearInput,
    isContentVisible,
    ...inputProps
}: InputFieldProps) {
    const fallbackRef = useRef<HTMLInputElement>(null);
    const inputRef = ref ?? fallbackRef;

    const [isInputContentVisible, setIsInputContentVisible] = useState<boolean>(
        isContentVisible ?? inputProps.type !== InputFieldType.Password,
    );

    useEffect(() => {
        if (isContentVisible !== undefined) {
            setIsInputContentVisible(isContentVisible);
        }
    }, [isContentVisible]);

    function onToggleButtonClick() {
        setIsInputContentVisible((prev) => !prev);
    }

    function focusInput() {
        if (inputRef?.current) {
            inputRef?.current?.focus();
        }
    }

    return (
        <InputFieldWrapper
            label={label}
            caption={caption}
            disabled={disabled}
            errorMessage={errorMessage}
            amountCounter={amountCounter}
            required={required}
        >
            <div
                className={cx('relative flex flex-row items-center gap-x-3', BORDER_CLASSES)}
                onClick={focusInput}
            >
                {leadingIcon && (
                    <span className="text-neutral-10 dark:text-neutral-92">{leadingIcon}</span>
                )}

                <input
                    type={
                        inputProps.type === InputFieldType.Password && isInputContentVisible
                            ? 'text'
                            : inputProps.type
                    }
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
                    className={cx(
                        INPUT_CLASSES,
                        INPUT_TEXT_CLASSES,
                        INPUT_PLACEHOLDER_CLASSES,
                        INPUT_NUMBER_CLASSES,
                    )}
                />

                {supportingText && <SecondaryText noErrorStyles>{supportingText}</SecondaryText>}

                {(trailingElement ||
                    (inputProps.type === InputFieldType.Password &&
                        inputProps.isVisibilityToggleEnabled)) && (
                    <InputFieldTrailingElement
                        onClearInput={onClearInput}
                        onToggleButtonClick={onToggleButtonClick}
                        trailingElement={trailingElement}
                        isContentVisible={isInputContentVisible}
                    />
                )}
            </div>
        </InputFieldWrapper>
    );
}
