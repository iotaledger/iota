// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TriangleDown } from '@iota/apps-ui-icons';
import cx from 'classnames';
import { forwardRef, useEffect, useRef, useState } from 'react';
import { Dropdown } from '../dropdown/Dropdown';
import { SecondaryText } from '@/components/atoms/secondary-text';
import { InputWrapper, LabelHtmlTag } from '../input/InputWrapper';
import { ListItem, ButtonUnstyled } from '@/components/atoms';
import { DropdownPosition } from '../dropdown';
import { SelectSize } from './select.enums';

export type SelectOption =
    | string
    | { id: string; renderLabel: () => React.JSX.Element }
    | { id: string; label: React.ReactNode };

interface SelectProps
    extends Pick<React.HTMLProps<HTMLSelectElement>, 'disabled'>,
        React.AriaAttributes {
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
            ...ariaProps
        },
        ref,
    ) => {
        const [isOpen, setIsOpen] = useState<boolean>(false);
        const selectedValue = findValueByProps(value, options);
        const wrapperRef = useRef<HTMLDivElement>(null);

        const selectorText = selectedValue || placeholder;
        const selectPadding = size === SelectSize.Small ? 'px-sm' : 'px-md';
        const textSize = size === SelectSize.Small ? 'text-body-md' : 'text-body-lg';

        useEffect(() => {
            if (disabled && isOpen) {
                closeDropdown();
            }
        }, [disabled, isOpen]);

        useEffect(() => {
            if (!isOpen) return;

            const handleClickOutside = (e: MouseEvent) => {
                const target = e.target as Node;
                if (wrapperRef.current && !wrapperRef.current.contains(target)) {
                    closeDropdown();
                }
            };

            document.addEventListener('mousedown', handleClickOutside);

            return () => {
                document.removeEventListener('mousedown', handleClickOutside);
            };
        }, [isOpen]);

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
                <div className="relative flex w-full flex-col" ref={wrapperRef}>
                    <ButtonUnstyled
                        ref={ref}
                        onClick={onSelectorClick}
                        disabled={disabled}
                        aria-expanded={isOpen}
                        aria-haspopup="listbox"
                        className={cx(
                            'select-container select-border-color focus-visible:enabled:select-border-focus-color active:enabled:select-border-focus-color group-[.errored]:select-border-error-color group-[.opened]:select-border-focus-color [&:is(:focus,_:focus-visible,_:active)]:enabled:select-border-focus-color hover:enabled:select-border-hover-color flex flex-row items-center gap-x-3 rounded-lg border py-sm disabled:cursor-not-allowed [&_svg]:h-5 [&_svg]:w-5',
                            selectPadding,
                        )}
                        {...ariaProps}
                    >
                        {leadingIcon && <span className="select-icon-color">{leadingIcon}</span>}

                        <div className="flex w-full flex-row items-baseline gap-x-3">
                            {selectorText && (
                                <div
                                    className={cx(
                                        'select-label-color block w-full text-start',
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
                            className={cx('select-icon-color transition-transform', {
                                ' rotate-180': isOpen,
                            })}
                        />
                    </ButtonUnstyled>

                    {isOpen && (
                        <div
                            className="fixed left-0 top-0 z-[49] bg-transparent"
                            onClick={closeDropdown}
                        />
                    )}
                    {isOpen && (
                        <div
                            className={cx(
                                'absolute z-50 min-w-full animate-dropdown-show',
                                dropdownPosition === DropdownPosition.Top
                                    ? 'bottom-full origin-bottom'
                                    : 'top-full origin-top',
                            )}
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
                    )}
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
