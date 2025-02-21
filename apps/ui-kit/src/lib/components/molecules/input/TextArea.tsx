// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, useEffect, useState } from 'react';
import { InputWrapper, InputWrapperProps } from './InputWrapper';
import {
    BORDER_CLASSES,
    INPUT_CLASSES,
    INPUT_TEXT_CLASSES,
    INPUT_PLACEHOLDER_CLASSES,
} from './input.classes';
import cx from 'classnames';
import { ButtonUnstyled } from '../../atoms/button';
import { VisibilityOff, VisibilityOn } from '@iota/apps-ui-icons';

type TextAreaProps = Omit<
    React.TextareaHTMLAttributes<HTMLTextAreaElement>,
    'cols' | 'resize' | 'className'
>;

interface TextFieldBaseProps extends TextAreaProps, InputWrapperProps {
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
     * If true the textarea is resizable vertically
     */
    isResizeEnabled?: boolean;
}

export const TextArea = forwardRef<HTMLTextAreaElement, TextFieldBaseProps>(function TextArea(
    {
        label,
        caption,
        disabled,
        errorMessage,
        value,
        amountCounter,
        isVisibilityToggleEnabled,
        isResizeEnabled,
        required,
        isContentVisible,
        ...textareaProps
    },
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
        <InputWrapper
            label={label}
            caption={caption}
            disabled={disabled || !isInputContentVisible}
            errorMessage={errorMessage}
            amountCounter={amountCounter}
            required={required}
        >
            <div className="relative">
                <textarea
                    disabled={disabled || !isInputContentVisible}
                    required={required}
                    ref={ref}
                    className={cx(
                        'peer block min-h-[50px]',
                        BORDER_CLASSES,
                        INPUT_CLASSES,
                        INPUT_TEXT_CLASSES,
                        INPUT_PLACEHOLDER_CLASSES,
                        isInputContentVisible && isResizeEnabled ? 'resize-y' : 'resize-none',
                        !isInputContentVisible &&
                            'not-visible select-none text-transparent dark:text-transparent',
                    )}
                    value={isInputContentVisible ? value : ''}
                    {...textareaProps}
                />
                {!isInputContentVisible && (
                    <div className="absolute left-0 top-0 flex h-full w-full flex-col items-stretch gap-y-2 px-md py-sm peer-[.not-visible]:select-none">
                        <VisibilityOffBar rows={3} halfWidthRow={3} />
                    </div>
                )}

                {isVisibilityToggleEnabled && (
                    <span className="absolute bottom-4 right-4 flex">
                        <ButtonUnstyled
                            onClick={onToggleButtonClick}
                            className="text-neutral-10 dark:text-neutral-92 [&_svg]:h-5 [&_svg]:w-5"
                            tabIndex={-1}
                        >
                            {isInputContentVisible ? <VisibilityOn /> : <VisibilityOff />}
                        </ButtonUnstyled>
                    </span>
                )}
            </div>
        </InputWrapper>
    );
});

interface VisibilityOffBarProps {
    rows: number;
    halfWidthRow?: number;
}

function VisibilityOffBar({ rows, halfWidthRow }: VisibilityOffBarProps) {
    return (
        <>
            {Array.from({ length: rows }).map((_, index) => {
                const isHalfWidth = halfWidthRow === index + 1;
                const width = isHalfWidth ? 'w-1/2' : 'w-full';
                return (
                    <div
                        key={index}
                        className={`h-2.5 rounded bg-neutral-92/60 dark:bg-neutral-10/60 ${width}`}
                    />
                );
            })}
        </>
    );
}
