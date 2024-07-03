// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

interface DropdownProps<T> {
    options: T[];
    value: T | null | undefined;
    onChange: (selectedOption: T) => void;
    placeholder?: string;
    disabled?: boolean;
    valueFromOption: (option: T) => string | number;
}

function Dropdown<T>({
    options,
    value,
    onChange,
    placeholder,
    disabled = false,
    valueFromOption: getValue,
}: DropdownProps<T>): JSX.Element {
    function handleSelectionChange(e: React.ChangeEvent<HTMLSelectElement>): void {
        const selectedKey = e.target.value;
        const selectedOption = options.find((option) => getValue(option) === selectedKey);
        if (selectedOption) {
            onChange(selectedOption);
        }
    }

    return (
        <select
            value={value ? getValue(value) : ''}
            onChange={handleSelectionChange}
            className="px-2 py-3"
            disabled={disabled}
        >
            {placeholder && (
                <option value="" disabled>
                    {placeholder}
                </option>
            )}

            {options.map((option, index) => (
                <option key={index} value={getValue(option)}>
                    {getValue(option)}
                </option>
            ))}
        </select>
    );
}

export default Dropdown;
