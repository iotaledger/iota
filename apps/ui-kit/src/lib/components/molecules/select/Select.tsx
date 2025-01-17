// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TriangleDown } from '@iota/apps-ui-icons';
import cx from 'classnames';
import { forwardRef, useEffect, useState } from 'react';
import { Dropdown } from '../dropdown/Dropdown';
import { SecondaryText } from '../../atoms/secondary-text';
import { InputWrapper, LabelHtmlTag } from '../input/InputWrapper';
import { ButtonUnstyled } from '../../atoms/button';
import { ListItem } from '../../atoms';
import { DropdownPosition } from '../dropdown';
import { SelectSize } from './select.enums';

export type SelectOption =
    | string
    | { id: string; renderLabel: () => React.JSX.Element }
    | { id: string; label: React.ReactNode };

interface SelectProps extends Pick<React.HTMLProps<HTMLSelectElement>, 'disabled'> {
    /**
     * The selected option value.
     */
    value?: string;
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
     * Placeholder for the selector
     */
    placeholder?: SelectOption;
    /**
     * The callback to call when the value changes.
     */
    onValueChange?: (id: string) => void;
    /**
     * The callback to call when the option is clicked.
     */
    onOptionClick?: (id: string) => void;
    /**
     * The dropdown position
     */
    dropdownPosition?: DropdownPosition;
    /**
     * The size of the select.
     */
    size?: SelectSize;
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
            placeholder,
            onValueChange,
            onOptionClick,
            value,
            dropdownPosition = DropdownPosition.Bottom,
            size = SelectSize.Default,
        },
        ref,
    ) => {
        const [isOpen, setIsOpen] = useState<boolean>(false);
        const selectedValue = findValueByProps(value, options);

        const selectorText = selectedValue || placeholder;
        const selectPadding = size === SelectSize.Small ? 'px-sm' : 'px-md';
        const textSize = size === SelectSize.Small ? 'text-body-md' : 'text-body-lg';
        useEffect(() => {
            if (disabled && isOpen) {
                closeDropdown();
            }
        }, [disabled, isOpen]);

        function findValueByProps(value: SelectProps['value'], options: SelectOption[] = []) {
            return (
                options.find((option) =>
                    typeof option === 'string' ? option === value : option.id === value,
                ) ?? options[0]
            );
        }

        function onSelectorClick() {
            setIsOpen((prev) => !prev);
        }

        function handleOptionClick(option: SelectOption) {
            closeDropdown();
            const clickedOption = typeof option === 'string' ? option : option.id;
            onOptionClick?.(clickedOption);

            if (option !== selectedValue) {
                onValueChange?.(clickedOption);
            }
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
                        className={cx(
                            'flex flex-row items-center gap-x-3 rounded-lg border border-neutral-80 py-sm hover:enabled:border-neutral-50 focus-visible:enabled:border-primary-30 active:enabled:border-primary-30 disabled:cursor-not-allowed  group-[.errored]:border-error-30 group-[.opened]:border-primary-30 dark:border-neutral-20 dark:hover:enabled:border-neutral-60 dark:group-[.errored]:border-error-80 dark:group-[.opened]:border-primary-80 [&:is(:focus,_:focus-visible,_:active)]:enabled:border-primary-30 dark:[&:is(:focus,_:focus-visible,_:active)]:enabled:border-primary-80 [&_svg]:h-5 [&_svg]:w-5',
                            selectPadding,
                        )}
                    >
                        {leadingIcon && (
                            <span className="text-neutral-10 dark:text-neutral-92">
                                {leadingIcon}
                            </span>
                        )}

                        <div className="flex w-full flex-row items-baseline gap-x-3">
                            {selectorText && (
                                <div
                                    className={cx(
                                        'block w-full text-start text-neutral-10 dark:text-neutral-92',
                                        textSize,
                                    )}
                                >
                                    <OptionLabel option={selectorText} />
                                </div>
                            )}

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
                        className={cx('absolute z-50 min-w-full', {
                            hidden: !isOpen,
                            'top-full':
                                !dropdownPosition || dropdownPosition === DropdownPosition.Bottom,
                            'bottom-full': dropdownPosition === DropdownPosition.Top,
                        })}
                    >
                        <Dropdown>
                            {options.map((option) => {
                                const optionIsString = typeof option === 'string';
                                return (
                                    <ListItem
                                        onClick={() => handleOptionClick(option)}
                                        hideBottomBorder
                                        key={optionIsString ? option : option.id}
                                    >
                                        <OptionLabel option={option} />
                                    </ListItem>
                                );
                            })}
                        </Dropdown>
                    </div>
                </div>
            </InputWrapper>
        );
    },
);

function OptionLabel({ option }: { option: SelectOption }) {
    if (typeof option === 'string') {
        return option;
    } else if ('renderLabel' in option) {
        return option.renderLabel();
    } else {
        return option.label;
    }
}
