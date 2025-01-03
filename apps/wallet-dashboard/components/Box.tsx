// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ReactNode } from 'react';

interface BoxProps {
    children: ReactNode;
    title?: string;
}

export function Box({ children, title }: BoxProps): JSX.Element {
    return (
        <div className="flex flex-col items-center gap-2 rounded-lg border border-white p-4">
            {title && <h2>{title}</h2>}
            {children}
        </div>
    );
}
