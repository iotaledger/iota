// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

interface BoxProps {
    children: React.ReactNode
    title?: string
}

function Box({ children, title }: BoxProps): JSX.Element {

	return (
        <div className="flex flex-col gap-2 items-center p-4 rounded-lg border border-white">
            {title && <h2>{title}</h2>}
            {children}
        </div>
    )
}

export default Box;
