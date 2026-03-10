// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close } from '@iota/apps-ui-icons';
import { ButtonUnstyled } from '@iota/apps-ui-kit';
import cx from 'clsx';
import { forwardRef, useRef } from 'react';

import { InputWrapper, InputWrapperProps } from './InputWrapper';
import { BORDER_GRADIENT_CLASSES, BORDER_LIGHT_CLASSES, INPUT_CLASSES } from './search.classes';

interface BaseInputProps extends InputWrapperProps {
    leadingIcon?: React.ReactNode;
    supportingText?: string;
    trailingElement?: React.ReactNode;
    value?: string;
    onClearInput?: () => void;
    lightSearch?: boolean;
}

type InputProps = BaseInputProps & Omit<React.InputHTMLAttributes<HTMLInputElement>, 'type'>;

export const SearchStylized = forwardRef<HTMLInputElement, InputProps>(function InputComponent(
    {
        disabled,
        errorMessage,
        leadingIcon,
        trailingElement,
        value,
        onClearInput,
        lightSearch,
        ...inputProps
    },
    forwardRef,
) {
    const inputWrapperRef = useRef<HTMLDivElement | null>(null);
    const borderClasses = lightSearch ? BORDER_LIGHT_CLASSES : BORDER_GRADIENT_CLASSES;
    const inputTextSize = lightSearch
        ? 'text-title-md xs:text-title-lg'
        : 'text-title-lg xs:text-headline-sm sm:text-headline-md';
    const placeholderTextColor = lightSearch
        ? 'placeholder:text-names-neutral-70'
        : 'input-placeholder-color';
    function focusOnInput() {
        inputWrapperRef.current?.querySelector('input')?.focus();
    }
    return (
        <InputWrapper disabled={disabled} errorMessage={errorMessage}>
            <div
                className={cx(
                    'relative flex flex-row items-center sm:px-lg sm:py-md py-xs rounded-full border-2 h-16',
                    lightSearch ? 'gap-x-3 px-sm' : 'gap-x-1 px-[14px]',
                    !lightSearch && 'search-container',
                    borderClasses,
                )}
                onClick={focusOnInput}
                ref={inputWrapperRef}
            >
                {leadingIcon && <span className="text-names-neutral-50">{leadingIcon}</span>}
                <div className="flex flex-1 flex-col items-start">
                    <input
                        ref={forwardRef}
                        type="text"
                        value={value}
                        disabled={disabled}
                        className={cx(
                            inputTextSize,
                            placeholderTextColor,
                            INPUT_CLASSES,
                            'leading-tight',
                        )}
                        {...inputProps}
                    />
                </div>
                {!trailingElement && (
                    <ButtonUnstyled
                        className={cx(
                            'text-names-neutral-92 [&_svg]:h-5 [&_svg]:w-5 p-sm state-layer relative rounded-full',
                            !value?.length && 'hidden',
                        )}
                        onClick={onClearInput}
                        tabIndex={-1}
                        aria-label="Clear search"
                    >
                        <Close />
                    </ButtonUnstyled>
                )}

                {trailingElement}
            </div>
        </InputWrapper>
    );
});
