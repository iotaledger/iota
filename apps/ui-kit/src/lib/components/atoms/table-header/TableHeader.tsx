// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

interface TableHeaderProps {
    /**
     * The text to display.
     */
    text: string;
}

export function TableHeader({ text }: TableHeaderProps): React.JSX.Element {
    return (
        <div className="state-layer relative flex w-full shrink-0 flex-row items-center justify-between bg-neutral-96 px-md py-sm dark:bg-neutral-10 hover:dark:bg-neutral-10">
            <span className="text-label-lg text-neutral-40 dark:text-neutral-60">{text}</span>
        </div>
    );
}
