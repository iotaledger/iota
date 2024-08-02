// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, useEffect, useState } from 'react';
import { TextFieldWrapper, TextFieldWrapperProps } from './TextFieldWrapper';
import { TextFieldTrailingElement } from './TextFieldTrailingElement';
import {
    BORDER_CLASSES,
    INPUT_CLASSES,
    INPUT_TEXT_CLASSES,
    PLACEHOLDER_TEXT_CLASSES,
} from './text-field.classes';
import cx from 'classnames';

type TextAreaProps = Omit<
    React.TextareaHTMLAttributes<HTMLTextAreaElement>,
    'cols' | 'resize' | 'className'
>;

interface TextFieldBaseProps extends TextAreaProps, TextFieldWrapperProps {
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
     * Amount counter that is shown at the side of the caption text.
     */
    amountCounter?: string | number;
    /**
     * Shows toggle button to show/hide the content of the input field
     */
    isVisibilityToggleEnabled?: boolean;
    /**
     * Is the content of the input visible
     */
    isContentVisible?: boolean;
    /**
     * Value of the input field
     */
    value?: string;
    /**
     * If true the textarea is resizable vertically
     */
    isResizeEnabled?: boolean;
}

export const TextArea = forwardRef<HTMLTextAreaElement, TextFieldBaseProps>(function TextArea(
    {
        name,
        label,
        placeholder,
        caption,
        disabled,
        errorMessage,
        value,
        amountCounter,
        isVisibilityToggleEnabled,
        isResizeEnabled,
        rows = 3,
        autoFocus,
        required,
        maxLength,
        minLength,
        isContentVisible,
        id,
        ...restProps
    }: TextFieldBaseProps,
    ref,
) {
    const [isInputContentVisible, setIsInputContentVisible] = useState<boolean>(
        isContentVisible ?? true,
    );

    useEffect(() => {
        setIsInputContentVisible(isContentVisible ?? true);
    }, [isContentVisible]);

    function onToggleButtonClick() {
        setIsInputContentVisible((prev) => !prev);
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
            <div className="relative">
                <textarea
                    disabled={disabled || !isInputContentVisible}
                    placeholder={placeholder}
                    required={required}
                    id={id}
                    name={name}
                    rows={rows}
                    autoFocus={autoFocus}
                    ref={ref}
                    className={cx(
                        'peer block min-h-[50px]',
                        BORDER_CLASSES,
                        INPUT_CLASSES,
                        INPUT_TEXT_CLASSES,
                        PLACEHOLDER_TEXT_CLASSES,
                        isInputContentVisible && isResizeEnabled ? 'resize-y' : 'resize-none',
                        !isInputContentVisible &&
                            'not-visible select-none text-transparent dark:text-transparent',
                    )}
                    value={isInputContentVisible ? value : ''}
                    maxLength={maxLength}
                    minLength={minLength}
                    {...restProps}
                />
                {!isInputContentVisible && (
                    <div className="absolute left-0 top-0 flex h-full w-full flex-col items-stretch gap-y-1 px-md py-sm peer-[.not-visible]:select-none">
                        <div className="h-full w-full rounded bg-neutral-92/60 dark:bg-neutral-10/60" />
                    </div>
                )}
                {isVisibilityToggleEnabled && (
                    <span className="absolute bottom-4 right-4 flex">
                        <TextFieldTrailingElement
                            onToggleButtonClick={onToggleButtonClick}
                            isContentVisible={isInputContentVisible}
                        />
                    </span>
                )}
            </div>
        </TextFieldWrapper>
    );
});
