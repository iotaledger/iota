// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ComponentProps, forwardRef, Fragment, useEffect, useRef, useState } from 'react';
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
import { SecondaryText } from '../../atoms/secondary-text';
import { Close, VisibilityOff, VisibilityOn } from '@iota/ui-icons';
import { ButtonUnstyled } from '../../atoms/button';
import { InputPropsByType, NumberInputProps } from './input.types';
import { NumericFormat } from 'react-number-format';

export type GenericInputProps = Omit<
    React.InputHTMLAttributes<HTMLInputElement>,
    'type' | 'className' | 'ref' | 'value' | 'defaultValue'
>;

export interface BaseInputProps extends GenericInputProps, InputWrapperProps {
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

type InputProps = BaseInputProps & InputPropsByType;

export const Input = forwardRef<HTMLInputElement, InputProps>(function InputComponent(
    {
        name,
        label,
        placeholder,
        caption,
        disabled,
        errorMessage,
        value,
        leadingIcon,
        supportingText,
        amountCounter,
        pattern,
        autoFocus,
        trailingElement,
        onClearInput,
        isContentVisible,
        isVisibilityToggleEnabled,
        ...inputProps
    },
    forwardRef,
) {
    isVisibilityToggleEnabled ??= inputProps.type === InputType.Password;
    const inputWrapperRef = useRef<HTMLDivElement | null>(null);

    const [hasBlurred, setHasBlurred] = useState<boolean>(false);

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
        if (inputWrapperRef.current) {
            inputWrapperRef.current.querySelector('input')?.focus();
        }
    }

    function handleBlur() {
        setHasBlurred(true);
    }

    const inputElementProps = {
        ...inputProps,
        name,
        placeholder,
        disabled,
        value,
        pattern,
        autoFocus,
        onBlur: handleBlur,
        className: cx(
            INPUT_CLASSES,
            INPUT_TEXT_CLASSES,
            INPUT_PLACEHOLDER_CLASSES,
            INPUT_NUMBER_CLASSES,
        ),
    };

    return (
        <InputWrapper
            label={label}
            caption={caption}
            disabled={disabled}
            errorMessage={hasBlurred && errorMessage ? errorMessage : ''}
            amountCounter={amountCounter}
            required={inputProps.required}
        >
            <div
                className={cx('relative flex flex-row items-center gap-x-3', BORDER_CLASSES)}
                onClick={focusInput}
                ref={inputWrapperRef}
            >
                {leadingIcon && (
                    <span className="text-neutral-10 dark:text-neutral-92">{leadingIcon}</span>
                )}
                <InputElement {...inputElementProps} inputRef={forwardRef} />

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
});

function InputElement({
    type,
    inputRef,
    ...inputProps
}: InputProps & {
    inputRef: React.ForwardedRef<HTMLInputElement>;
}) {
    return type !== InputType.Number ? (
        <input {...inputProps} type={type} ref={inputRef} />
    ) : (
        <NumericInput inputRef={inputRef} {...inputProps} type={type} />
    );
}

function NumericInput({
    type,
    onValueChange,
    inputRef,
    ...inputProps
}: GenericInputProps &
    NumberInputProps & {
        inputRef: React.ForwardedRef<HTMLInputElement>;
    }) {
    const numericFormatProps: ComponentProps<typeof NumericFormat> = {
        decimalScale: inputProps.decimals ? undefined : 0,
        thousandSeparator: true,
        onChange: (e) => inputProps.onChange?.(e),
        onValueChange: (values) => onValueChange?.(values.value),
    };
    return <NumericFormat getInputRef={inputRef} {...numericFormatProps} {...inputProps} />;
}

function InputTrailingElement({
    value,
    type,
    onClearInput,
    isContentVisible,
    trailingElement,
    onToggleButtonClick,
}: BaseInputProps & {
    onToggleButtonClick: (e: React.MouseEvent<HTMLButtonElement>) => void;
    type: InputProps['type'];
}) {
    const showClearInput = Boolean(type === InputType.Text && value && onClearInput);
    const showPasswordToggle = Boolean(type === InputType.Password && onToggleButtonClick);
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
