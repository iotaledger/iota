// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode } from 'react';

interface HeaderProps {
    middleContent?: ReactNode;
    rightContent?: ReactNode;
    leftContent?: ReactNode;
}

/**
 * General page header that can render arbitrary content where the content
 * located in the middle of the header is centered and has a capped width
 */
export function Header({ leftContent, middleContent, rightContent }: HeaderProps) {
    return (
        <header className="flex flex-row items-center justify-between bg-neutral-100 px-md py-xs dark:bg-neutral-6">
            {leftContent && <div>{leftContent}</div>}
            {middleContent && <div className="shrink-0 overflow-hidden">{middleContent}</div>}
            {rightContent && <div>{rightContent}</div>}
        </header>
    );
}
