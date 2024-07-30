// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, useEffect, useRef, useState } from 'react';
import { TextFieldType } from './text-field.enums';
import { TextFieldTrailingElement } from './TextFieldTrailingElement';
import cx from 'classnames';
import { TextFieldWrapper, type TextFieldWrapperProps, SecondaryText } from './TextFieldWrapper';
import {
    BORDER_CLASSES,
    INPUT_CLASSES,
    INPUT_TEXT_CLASSES,
    PLACEHOLDER_TEXT_CLASSES,
} from './text-field.classes';

type InputProps = Omit<React.InputHTMLAttributes<HTMLInputElement>, 'type' | 'className'>;

export interface TextFieldProps extends InputProps, TextFieldWrapperProps {
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
    /**
     * Shows toggle button to show/hide the content of the input field
     */
    isVisibilityToggleEnabled?: boolean;
    /**
     * Type of the input field
     */
    type: TextFieldType;
}

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
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
    const inputRef = useRef<HTMLInputElement | null>(null);

    const [isInputContentVisible, setIsInputContentVisible] = useState<boolean>(
        isContentVisible ?? inputProps.type !== TextFieldType.Password,
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
        <TextFieldWrapper
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
                    name={name}
                    placeholder={placeholder}
                    disabled={disabled}
                    value={value}
                    ref={assignRefs}
                    pattern={pattern}
                    autoFocus={autoFocus}
                    className={cx(INPUT_CLASSES, INPUT_TEXT_CLASSES, PLACEHOLDER_TEXT_CLASSES)}
                    {...inputProps}
                    type={
                        inputProps.type === TextFieldType.Password && isInputContentVisible
                            ? 'text'
                            : inputProps.type
                    }
                />

                {supportingText && <SecondaryText noErrorStyles>{supportingText}</SecondaryText>}

                {(trailingElement ||
                    (inputProps.type === TextFieldType.Password && isVisibilityToggleEnabled)) && (
                    <TextFieldTrailingElement
                        onClearInput={onClearInput}
                        onToggleButtonClick={onToggleButtonClick}
                        isContentVisible={isInputContentVisible}
                        trailingElement={trailingElement}
                    />
                )}
            </div>
        </TextFieldWrapper>
    );
});
