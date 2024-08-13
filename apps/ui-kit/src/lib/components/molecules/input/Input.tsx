// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, Fragment, useEffect, useRef, useState } from 'react';
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
import { ButtonUnstyled } from '../../atoms/button/ButtonUnstyled';

type InputPickedProps = Omit<
    React.InputHTMLAttributes<HTMLInputElement>,
    'type' | 'className' | 'ref'
>;

export interface InputProps extends InputPickedProps, InputWrapperProps {
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
    /**
     * Shows toggle button to show/hide the content of the input field
     */
    isVisibilityToggleEnabled?: boolean;
    /**
     * Type of the input field
     */
    type: InputType;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
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
    ref,
) {
    isVisibilityToggleEnabled ??= inputProps.type === InputType.Password;
    const inputRef = useRef<HTMLInputElement | null>(null);

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
        if (inputRef.current) {
            inputRef.current.focus();
        }
    }

    function assignRefs(element: HTMLInputElement) {
        if (ref) {
            if (typeof ref === 'function') {
                ref(element);
            } else {
                ref.current = element;
            }
        }
        inputRef.current = element;
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
                onClick={focusInput}
            >
                {leadingIcon && (
                    <span className="text-neutral-10 dark:text-neutral-92">{leadingIcon}</span>
                )}

                <input
                    {...inputProps}
                    name={name}
                    placeholder={placeholder}
                    disabled={disabled}
                    value={value}
                    ref={assignRefs}
                    pattern={pattern}
                    autoFocus={autoFocus}
                    type={
                        inputProps.type === InputType.Password && isInputContentVisible
                            ? 'text'
                            : inputProps.type
                    }
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
});

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

    const ICON_WIDTH_HEIGHT = 20;

    if (showClearInput) {
        return (
            <ButtonUnstyled className="text-neutral-10 dark:text-neutral-92" onClick={onClearInput}>
                <Close width={ICON_WIDTH_HEIGHT} height={ICON_WIDTH_HEIGHT} />
            </ButtonUnstyled>
        );
    } else if (showPasswordToggle) {
        return (
            <ButtonUnstyled
                onClick={onToggleButtonClick}
                className="text-neutral-10 dark:text-neutral-92"
            >
                {isContentVisible ? (
                    <VisibilityOn width={ICON_WIDTH_HEIGHT} height={ICON_WIDTH_HEIGHT} />
                ) : (
                    <VisibilityOff width={ICON_WIDTH_HEIGHT} height={ICON_WIDTH_HEIGHT} />
                )}
            </ButtonUnstyled>
        );
    } else if (showTrailingElement) {
        return <Fragment>{trailingElement}</Fragment>;
    }
}
