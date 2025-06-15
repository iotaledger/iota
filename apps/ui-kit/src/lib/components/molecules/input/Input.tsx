// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, Fragment, useEffect, useRef, useState } from 'react';
import cx from 'classnames';
import type { InputWrapperProps } from './InputWrapper';
import { InputWrapper } from './InputWrapper';
import {
    BORDER_CLASSES,
    INPUT_CLASSES,
    INPUT_TEXT_CLASSES,
    INPUT_NUMBER_CLASSES,
    INPUT_PLACEHOLDER_CLASSES,
} from './input.classes';
import { InputType } from './input.enums';
import { SecondaryText } from '@/components/atoms/secondary-text';
import { Close, VisibilityOff, VisibilityOn } from '@iota/apps-ui-icons';
import { ButtonUnstyled } from '@/components/atoms/button';
import type { InputPropsByType, NumericFormatInputProps } from './input.types';
import { NumericFormat } from 'react-number-format';

export interface BaseInputProps extends InputWrapperProps {
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
     * Is the content of the input visible
     */
    isContentVisible?: boolean;
    /**
     * Value of the input
     */
    value?: string | number;
    /**
     * Default value of the input
     */
    defaultValue?: string | number;
    /**
     * onClearInput function that is called when the clear button is clicked
     */
    onClearInput?: () => void;
    /**
     * Shows toggle button to show/hide the content of the input field
     */
    isVisibilityToggleEnabled?: boolean;
}

export type InputProps = BaseInputProps & InputPropsByType;

export const Input = forwardRef<HTMLInputElement, InputProps>(function InputComponent(
    {
        label,
        caption,
        disabled,
        errorMessage,
        leadingIcon,
        supportingText,
        amountCounter,
        trailingElement,
        isContentVisible,
        value,
        defaultValue,
        onClearInput,
        isVisibilityToggleEnabled,
        type,
        ...inputProps
    },
    forwardRef,
) {
    isVisibilityToggleEnabled ??= type === InputType.Password;
    const inputWrapperRef = useRef<HTMLDivElement | null>(null);

    const [isInputContentVisible, setIsInputContentVisible] = useState<boolean>(
        isContentVisible ?? type !== InputType.Password,
    );

    useEffect(() => {
        setIsInputContentVisible(isContentVisible ?? type !== InputType.Password);
    }, [type, isContentVisible]);

    function onToggleButtonClick() {
        setIsInputContentVisible((prev) => !prev);
    }

    function focusOnInput() {
        if (inputWrapperRef.current) {
            inputWrapperRef.current.querySelector('input')?.focus();
        }
    }

    return (
        <InputWrapper
            label={label}
            caption={caption}
            disabled={disabled}
            errorMessage={errorMessage}
            amountCounter={amountCounter}
            required={inputProps.required}
        >
            <div
                className={cx('relative flex flex-row items-center gap-x-3', BORDER_CLASSES)}
                onClick={focusOnInput}
                ref={inputWrapperRef}
            >
                {leadingIcon && (
                    <span className="text-neutral-10 dark:text-neutral-92">{leadingIcon}</span>
                )}
                <InputElement
                    {...inputProps}
                    inputRef={forwardRef}
                    value={value}
                    type={
                        type === InputType.Password && isInputContentVisible ? InputType.Text : type
                    }
                    disabled={disabled}
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
                    type={type}
                    onClearInput={onClearInput}
                    isContentVisible={isInputContentVisible}
                    trailingElement={trailingElement}
                    isVisibilityToggleEnabled={isVisibilityToggleEnabled}
                    onToggleButtonClick={onToggleButtonClick}
                />
            </div>
        </InputWrapper>
    );
});

function InputElement({
    type,
    inputRef,
    ...inputProps
}: InputProps & {
    inputRef: React.ForwardedRef<HTMLInputElement>;
    className: string;
}) {
    function preventScrollInputChange(e: React.WheelEvent<HTMLInputElement>) {
        if (type === InputType.Number) {
            const input = e.currentTarget;

            input.blur();
            e.stopPropagation();
            setTimeout(() => {
                input.focus({ preventScroll: true });
            }, 0);
        }
    }
    return type === InputType.NumericFormat ? (
        <NumericFormatInput inputRef={inputRef} {...inputProps} type={type} />
    ) : (
        <input
            ref={inputRef}
            {...inputProps}
            type={type}
            onWheel={(e) => {
                preventScrollInputChange(e);
                inputProps.onWheel?.(e);
            }}
        />
    );
}

function NumericFormatInput({
    inputRef,
    className,
    type,
    ...inputProps
}: NumericFormatInputProps &
    InputProps & {
        inputRef: React.ForwardedRef<HTMLInputElement>;
        className: string;
        value?: string | number;
    }) {
    return (
        <NumericFormat
            className={className}
            valueIsNumericString
            getInputRef={inputRef}
            {...inputProps}
        />
    );
}

function InputTrailingElement({
    value,
    type,
    onClearInput,
    isContentVisible,
    trailingElement,
    onToggleButtonClick,
    isVisibilityToggleEnabled,
}: BaseInputProps & {
    onToggleButtonClick: (e: React.MouseEvent<HTMLButtonElement>) => void;
    type: InputPropsByType['type'];
}) {
    const showClearInput = Boolean(type === InputType.Text && value && onClearInput);
    const showPasswordToggle = Boolean(type === InputType.Password && isVisibilityToggleEnabled);
    const showTrailingElement = Boolean(trailingElement && !showClearInput && !showPasswordToggle);

    if (showClearInput) {
        return (
            <ButtonUnstyled
                className="text-neutral-10 dark:text-neutral-92 [&_svg]:h-5 [&_svg]:w-5"
                onClick={onClearInput}
                tabIndex={-1}
            >
                <Close />
            </ButtonUnstyled>
        );
    } else if (showPasswordToggle) {
        return (
            <ButtonUnstyled
                onClick={onToggleButtonClick}
                className="text-neutral-10 dark:text-neutral-92 [&_svg]:h-5 [&_svg]:w-5"
                tabIndex={-1}
            >
                {isContentVisible ? <VisibilityOn /> : <VisibilityOff />}
            </ButtonUnstyled>
        );
    } else if (showTrailingElement) {
        return <Fragment>{trailingElement}</Fragment>;
    }
}
