// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

export interface ButtonProps {
    label: string;
}

export function Button({ label }: ButtonProps): React.JSX.Element {
    return (
        <button className="rounded-full bg-primary-40 px-xs py-xxs text-white dark:bg-primary-70 dark:text-neutral-20">
            {label}
        </button>
    );
}
