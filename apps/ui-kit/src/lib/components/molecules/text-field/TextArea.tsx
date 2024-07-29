// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from 'react';
import { TextFieldWrapper, TextFieldWrapperProps } from './TextFieldWrapper';
import { TextFieldTrailingElement } from './TextFieldTrailingElement';
import {
    BORDER_CLASSES,
    INPUT_CLASSES,
    INPUT_TEXT_CLASSES,
    PLACEHOLDER_TEXT_CLASSES,
    RESIZE_CLASSES,
} from './text-field.classes';
import cx from 'classnames';
import { Resize } from './text-field.enums';

type InputPickedProps = Pick<
    React.TextareaHTMLAttributes<HTMLTextAreaElement>,
    | 'maxLength'
    | 'minLength'
    | 'rows'
    | 'cols'
    | 'autoFocus'
    | 'name'
    | 'required'
    | 'placeholder'
    | 'disabled'
    | 'id'
>;

interface TextFieldBaseProps extends InputPickedProps, TextFieldWrapperProps {
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
     * Amount counter that is shown at the side of the caption text.
     */
    amountCounter?: string | number;
    /**
     * Shows toggle button to show/hide the content of the input field
     */
    isVisibilityToggleEnabled?: boolean;
    /**
     * Ref for the input field
     */
    ref?: React.RefObject<HTMLTextAreaElement>;
    /**
     * Is the content of the input visible
     */
    isContentVisible?: boolean;
    /**
     * Value of the input field
     */
    value?: string;
    /**
     * Resize property of the textarea
     */
    resize?: Resize;
}

export function TextArea({
    name,
    label,
    placeholder,
    caption,
    disabled,
    errorMessage,
    onChange,
    value,
    amountCounter,
    isVisibilityToggleEnabled,
    resize = Resize.Vertical,
    rows = 3,
    cols,
    autoFocus,
    required,
    maxLength,
    minLength,
    isContentVisible,
    ref,
    id,
}: TextFieldBaseProps) {
    const fallbackRef = useRef<HTMLTextAreaElement>(null);
    const inputRef = ref ?? fallbackRef;

    const [isInputContentVisible, setIsInputContentVisible] = useState<boolean>(
        isContentVisible ?? true,
    );

    useEffect(() => {
        setIsInputContentVisible(isContentVisible ?? true);
    }, [isContentVisible]);

    function onToggleButtonClick() {
        setIsInputContentVisible((prev) => !prev);
    }

    function handleOnChange(e: React.ChangeEvent<HTMLTextAreaElement>) {
        if (isInputContentVisible) {
            onChange?.(e.target.value, e.target.name);
        }
    }

    return (
        <TextFieldWrapper
            label={label}
            caption={caption}
            disabled={disabled}
            errorMessage={errorMessage}
            amountCounter={amountCounter}
            required={required}
        >
            <div className="flex">
                <span className="relative">
                    <textarea
                        disabled={disabled || !isInputContentVisible}
                        placeholder={placeholder}
                        required={required}
                        id={id}
                        name={name}
                        rows={rows}
                        cols={cols}
                        autoFocus={autoFocus}
                        ref={inputRef}
                        onChange={handleOnChange}
                        className={cx(
                            'peer',
                            BORDER_CLASSES,
                            INPUT_CLASSES,
                            INPUT_TEXT_CLASSES,
                            PLACEHOLDER_TEXT_CLASSES,
                            isInputContentVisible ? RESIZE_CLASSES[resize] : 'resize-none',
                            !isInputContentVisible &&
                                'not-visible select-none text-transparent dark:text-transparent',
                        )}
                        value={isInputContentVisible ? value : ''}
                        maxLength={maxLength}
                        minLength={minLength}
                    />
                    {!isInputContentVisible && (
                        <div className="absolute left-0 top-0 flex h-full w-full flex-col items-stretch gap-y-1 px-md py-sm peer-[.not-visible]:select-none">
                            <div className="h-full w-full rounded bg-neutral-92/60 dark:bg-neutral-10/60" />
                        </div>
                    )}
                    {isVisibilityToggleEnabled && (
                        <div className="absolute bottom-4 right-4 flex">
                            <TextFieldTrailingElement
                                onToggleButtonClick={onToggleButtonClick}
                                isContentVisible={isInputContentVisible}
                            />
                        </div>
                    )}
                </span>
            </div>
        </TextFieldWrapper>
    );
}
