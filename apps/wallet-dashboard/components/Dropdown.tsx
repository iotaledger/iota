// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

interface DropdownProps<T> {
    options: T[];
    value: T | null | undefined;
    onChange: (selectedOption: T) => void;
    placeholder?: string;
    keyFromOption: (option: T) => string | number;
    labelFromOption: (option: T) => React.ReactNode;
}

function Dropdown<T>({
    options,
    value,
    onChange,
    placeholder,
    keyFromOption: getKey,
    labelFromOption: getLabel,
}: DropdownProps<T>): JSX.Element {
    const handleSelectionChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
        const selectedKey = e.target.value;
        const selectedOption = options.find((option) => getKey(option) === selectedKey);
        if (selectedOption) {
            onChange(selectedOption);
        }
    };

    return (
        <select
            value={value ? getKey(value) : ''}
            onChange={handleSelectionChange}
            className="px-2 py-3"
        >
            {placeholder && (
                <option value="" disabled>
                    {placeholder}
                </option>
            )}

            {options.map((option, index) => (
                <option key={index} value={getKey(option)}>
                    {getLabel(option)}
                </option>
            ))}
        </select>
    );
}

export default Dropdown;
