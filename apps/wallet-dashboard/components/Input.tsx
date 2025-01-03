// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

interface InputProps {
    label?: string;
    value: string;
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
    placeholder?: string;
    type?: React.HTMLInputTypeAttribute;
    error?: string;
}

export function Input({
    label,
    value,
    onChange,
    placeholder,
    type = 'text',
    error,
}: InputProps): JSX.Element {
    return (
        <div className="flex flex-col gap-1">
            {label && <label>{label}</label>}
            <input
                type={type}
                value={value}
                onChange={onChange}
                placeholder={placeholder}
                className="w-full rounded-lg border border-gray-400 p-2"
            />
            {error && <span className="text-red-500">{error}</span>}
        </div>
    );
}
