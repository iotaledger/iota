// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface ButtonProps {
    onClick?: (event: React.MouseEvent<HTMLButtonElement, MouseEvent>) => void;
    children: React.ReactNode;
    disabled?: boolean;
}

export function Button({ onClick, children, disabled }: ButtonProps): JSX.Element {
    return (
        <button
            onClick={onClick}
            className="rounded-lg bg-gray-200 px-4 py-2 text-black disabled:cursor-not-allowed disabled:opacity-50"
            disabled={disabled}
        >
            {children}
        </button>
    );
}
