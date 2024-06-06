// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

interface ButtonProps {
    onClick?: (event: React.MouseEvent<HTMLButtonElement, MouseEvent>) => void;
    children: React.ReactNode;
    disabled?: boolean;
}

function Button({ onClick, children, disabled }: ButtonProps): JSX.Element {
    return (
        <button
            onClick={onClick}
            className="rounded-lg bg-gray-200 px-4 py-2 text-black"
            disabled={disabled}
        >
            {children}
        </button>
    );
}

export default Button;
