// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';
import { type ReactNode } from 'react';

interface ListItemProps {
    active?: boolean;
    children: ReactNode;
    onClick?(): void;
}

export function ListItem({ active, children, onClick }: ListItemProps): JSX.Element {
    return (
        <li className="list-none">
            <button
                type="button"
                className={clsx(
                    'text-body block w-full cursor-pointer rounded-md border px-2.5 py-2 text-left',
                    active
                        ? 'border-gray-40 bg-gray-40 font-semibold text-neutral-40 shadow-sm'
                        : 'border-transparent bg-white font-medium text-neutral-50',
                )}
                onClick={onClick}
            >
                {children}
            </button>
        </li>
    );
}

interface VerticalListProps {
    children: ReactNode;
}

export function VerticalList({ children }: VerticalListProps): JSX.Element {
    return <ul className="m-0 flex flex-col gap-1 p-0">{children}</ul>;
}
