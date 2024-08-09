// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Fragment, useEffect, useRef, useState } from 'react';
import cx from 'classnames';
import { InputWrapper, InputWrapperProps } from './InputWrapper';
import {
    BORDER_CLASSES,
    INPUT_CLASSES,
    INPUT_TEXT_CLASSES,
    INPUT_NUMBER_CLASSES,
    INPUT_PLACEHOLDER_CLASSES,
} from './input.classes';
import { InputType } from './input.enums';
import { InputPropsByType } from './input.types';
import { SecondaryText } from '../../atoms/secondary-text';
import { Close, VisibilityOff, VisibilityOn } from '@iota/ui-icons';
import { ButtonUnstyled } from '../../atoms/button/ButtonUnstyled';

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

interface InputBaseProps extends InputPickedProps, InputWrapperProps {
    /**
     * Callback function that is called when the input value changes
     */
    onChange?: (value: string, name?: string) => void;
    /**
     * A leading icon that is shown before the input
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
     * Trailing element that is shown after the input
     */
    trailingElement?: React.JSX.Element;
    /**
     * Ref for the input
     */
    ref?: React.RefObject<HTMLInputElement>;
    /**
     * Is the content of the input visible
     */
    isContentVisible?: boolean;
    /**
     * Value of the input
     */
    value?: string;
    /**
     * onClearInput function that is called when the clear button is clicked
     */
    onClearInput?: () => void;
}

export type InputProps = InputBaseProps & InputPropsByType;

export function Input({
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
}: InputProps) {
    const fallbackRef = useRef<HTMLInputElement>(null);
    const inputRef = ref ?? fallbackRef;

    const [isInputContentVisible, setIsInputContentVisible] = useState<boolean>(
        isContentVisible ?? inputProps.type !== InputType.Password,
    );

    useEffect(() => {
        setIsInputContentVisible(isContentVisible ?? inputProps.type !== InputType.Password);
    }, [inputProps.type, isContentVisible]);

    function onToggleButtonClick() {
        setIsInputContentVisible((prev) => !prev);
    }

    function focusInput() {
        if (inputRef?.current) {
            inputRef?.current?.focus();
        }
    }

    return (
        <InputWrapper
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
                        inputProps.type === InputType.Password && isInputContentVisible
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

                {supportingText && <SecondaryText>{supportingText}</SecondaryText>}
                <InputTrailingElement
                    value={value}
                    type={inputProps.type}
                    onClearInput={onClearInput}
                    isContentVisible={isInputContentVisible}
                    trailingElement={trailingElement}
                    onToggleButtonClick={onToggleButtonClick}
                />
            </div>
        </InputWrapper>
    );
}

function InputTrailingElement({
    value,
    type,
    onClearInput,
    isContentVisible,
    trailingElement,
    onToggleButtonClick,
}: InputProps & { onToggleButtonClick: (e: React.MouseEvent<HTMLButtonElement>) => void }) {
    const showClearInput = Boolean(type === InputType.Text && value && onClearInput);
    const showPasswordToggle = Boolean(type === InputType.Password && onToggleButtonClick);
    const showTrailingElement = Boolean(trailingElement && !showClearInput && !showPasswordToggle);

    if (showClearInput) {
        return (
            <ButtonUnstyled className="text-neutral-10 dark:text-neutral-92" onClick={onClearInput}>
                <Close />
            </ButtonUnstyled>
        );
    } else if (showPasswordToggle) {
        return (
            <ButtonUnstyled
                onClick={onToggleButtonClick}
                className="text-neutral-10 dark:text-neutral-92"
            >
                {isContentVisible ? <VisibilityOn /> : <VisibilityOff />}
            </ButtonUnstyled>
        );
    } else if (showTrailingElement) {
        return <Fragment>{trailingElement}</Fragment>;
    }
}
