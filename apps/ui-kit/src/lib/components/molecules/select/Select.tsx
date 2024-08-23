// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TriangleDown } from '@iota/ui-icons';
import cx from 'classnames';
import { forwardRef, useEffect, useState } from 'react';
import { Dropdown } from '../dropdown/Dropdown';
import { SecondaryText } from '../../atoms/secondary-text';
import { InputWrapper, LabelHtmlTag } from '../input/InputWrapper';
import { ButtonUnstyled } from '../../atoms/button/ButtonUnstyled';
import { ListItem } from '../../atoms';

export type SelectOption = { id: string; displayElement: React.ReactNode; selected?: boolean };

interface SelectProps {
    /**
     * The field label.
     */
    label?: string;
    /**
     * The field caption.
     */
    caption?: string;
    /**
     * The dropdown elements to render.
     */
    options: SelectOption[];
    /**
     * The icon to show on the left of the field.
     */
    leadingIcon?: React.ReactNode;
    /**
     * The supporting text to shown at the end of the selector.
     */
    supportingText?: string;
    /**
     * The error message to show under the field.
     */
    errorMessage?: string;
    /**
     * The callback to call when the value changes.
     */
    onValueChange?: (option: SelectOption) => void;
    /**
     * Whether the field is disabled.
     */
    disabled?: boolean;
    /**
     * The rendered element in the select button
     */
    placeholder: React.ReactNode;
}

export const Select = forwardRef<HTMLButtonElement, SelectProps>(
    (
        {
            disabled,
            label,
            leadingIcon,
            supportingText,
            errorMessage,
            caption,
            options,
            onValueChange,
            placeholder,
        },
        ref,
    ) => {
        const [isOpen, setIsOpen] = useState<boolean>(false);

        useEffect(() => {
            if (disabled && isOpen) {
                closeDropdown();
            }
        }, [disabled, isOpen]);

        function onSelectorClick() {
            setIsOpen((prev) => !prev);
        }

        function handleOptionClick(clickedOption: SelectOption) {
            onValueChange?.(clickedOption);
            closeDropdown();
        }

        function closeDropdown() {
            setIsOpen(false);
        }

        return (
            <InputWrapper
                label={label}
                caption={caption}
                disabled={disabled}
                errorMessage={errorMessage}
                labelHtmlTag={LabelHtmlTag.Div}
            >
                <div className="relative flex w-full flex-col">
                    <ButtonUnstyled
                        ref={ref}
                        onClick={onSelectorClick}
                        disabled={disabled}
                        className="flex flex-row items-center gap-x-3 rounded-lg border border-neutral-80 px-md py-sm hover:enabled:border-neutral-50 focus-visible:enabled:border-primary-30 active:enabled:border-primary-30 disabled:cursor-not-allowed  group-[.errored]:border-error-30 group-[.opened]:border-primary-30 dark:border-neutral-20 dark:hover:enabled:border-neutral-60 dark:group-[.errored]:border-error-80 dark:group-[.opened]:border-primary-80 [&:is(:focus,_:focus-visible,_:active)]:enabled:border-primary-30 dark:[&:is(:focus,_:focus-visible,_:active)]:enabled:border-primary-80 [&_svg]:h-5 [&_svg]:w-5"
                    >
                        {leadingIcon && (
                            <span className="text-neutral-10 dark:text-neutral-92">
                                {leadingIcon}
                            </span>
                        )}

                        <div className="flex w-full flex-row items-baseline gap-x-3">
                            {placeholder}

                            {supportingText && (
                                <div className="ml-auto">
                                    <SecondaryText>{supportingText}</SecondaryText>
                                </div>
                            )}
                        </div>

                        <TriangleDown
                            className={cx(
                                'text-neutral-10 transition-transform dark:text-neutral-92',
                                {
                                    ' rotate-180': isOpen,
                                },
                            )}
                        />
                    </ButtonUnstyled>

                    {isOpen && (
                        <div
                            className="fixed left-0 top-0 z-[49] h-screen w-screen bg-transparent"
                            onClick={closeDropdown}
                        />
                    )}
                    <div
                        className={cx('absolute top-full z-50 min-w-full', {
                            hidden: !isOpen,
                        })}
                    >
                        <Dropdown>
                            {options.map((option) => (
                                <ListItem
                                    onClick={() => handleOptionClick(option)}
                                    hideBottomBorder
                                    key={option.id}
                                >
                                    {option.displayElement}
                                </ListItem>
                            ))}
                        </Dropdown>
                    </div>
                </div>
            </InputWrapper>
        );
    },
);
