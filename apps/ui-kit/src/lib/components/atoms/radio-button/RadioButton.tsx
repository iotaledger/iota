// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

// Define the RadioButtonProps type
type RadioButtonProps = {
    checked?: boolean;
    onChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
} & React.InputHTMLAttributes<HTMLInputElement>;

// Define the RadioButton component
const RadioButton: React.FC<RadioButtonProps> = ({ className, checked, onChange, ...props }) => {
    return (
        <input
            type="radio"
            checked={checked}
            onChange={onChange}
            className={`radio-button ${className || ''}`}
            {...props}
        />
    );
};

export { RadioButton };
