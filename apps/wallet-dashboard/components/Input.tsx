// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InputType } from '@/lib/enums';
import React from 'react';

interface InputProps {
    label?: string;
    value: string;
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
    placeholder?: string;
    type?: InputType;
}

function Input({
    label,
    value,
    onChange,
    placeholder,
    type = InputType.Text,
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
        </div>
    );
}

export default Input;
