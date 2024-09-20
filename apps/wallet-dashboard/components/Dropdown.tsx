// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

interface DropdownProps<T> {
    options: T[];
    selectedOption: T | null | undefined;
    onChange: (selectedOption: T) => void;
    placeholder?: string;
    disabled?: boolean;
    getOptionId: (option: T) => string | number;
}

function Dropdown<T>({
    options,
    selectedOption,
    onChange,
    placeholder,
    disabled = false,
    getOptionId,
}: DropdownProps<T>): JSX.Element {
    function handleSelectionChange(e: React.ChangeEvent<HTMLSelectElement>): void {
        const selectedKey = e.target.value;
        const selectedOption = options.find((option) => getOptionId(option) === selectedKey);
        if (selectedOption) {
            onChange(selectedOption);
        }
    }

    return (
        <select
            value={selectedOption ? getOptionId(selectedOption) : ''}
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
                <option key={index} value={getOptionId(option)}>
                    {getOptionId(option)}
                </option>
            ))}
        </select>
    );
}

export default Dropdown;
